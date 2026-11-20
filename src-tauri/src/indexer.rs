//! Parallel, uncapped folder indexer.
//! Walks with jwalk, reads EXIF / video headers in a rayon pool, and streams
//! batches into the SQLite catalog. Unchanged files (same path, size, mtime)
//! are skipped. Originals are never moved or rewritten.
//!
//! A perceptual hash is computed from an embedded EXIF thumbnail when one is
//! present. Full-decode hashing is left to the thumbnail pass so a metadata
//! index stays cheap. Near-duplicate *grouping* is Angen's job; this module
//! only stores the hash.

use crate::catalog::{self, CatalogReport};
use crate::exifmeta;
use crate::media::{classify_media_type, item_id_for, extension_kind, MediaRecord};
use crate::thumbs;
use jwalk::WalkDir;
use rayon::prelude::*;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

const BATCH: usize = 512;

#[derive(Debug, Serialize)]
pub struct PhotoEntry {
    pub path: String,
    pub name: String,
    pub bytes: u64,
    pub modified_ms: u64,
    pub kind: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ScanReport {
    pub root: String,
    pub count: usize,
    pub truncated: bool,
    pub files: Vec<PhotoEntry>,
}

/// Fast listing with no EXIF work and no file cap. Prefer `index_folder` plus
/// `catalog::page` for large libraries (the UI should not pull every path at once).
pub fn scan_folder(directory: &str) -> Result<ScanReport, String> {
    let root = std::fs::canonicalize(directory).map_err(|e| format!("Folder not available: {e}"))?;
    if !root.is_dir() {
        return Err("Not a folder".into());
    }
    let mut files = Vec::new();
    for item in WalkDir::new(&root).follow_links(false).into_iter().filter_map(|e| e.ok())
    {
        if !item.file_type().is_file() {
            continue;
        }
        let kind = match extension_kind(item.path().as_path()) {
            Some(kind) => kind,
            None => continue,
        };
        let meta = match item.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|v| v.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        files.push(PhotoEntry {
            path: item.path().to_string_lossy().into_owned(),
            name: item.file_name().to_string_lossy().into_owned(),
            bytes: meta.len(),
            modified_ms,
            kind,
        });
    }
    Ok(ScanReport {
        root: root.to_string_lossy().into_owned(),
        count: files.len(),
        truncated: false,
        files,
    })
}

struct Walked {
    path: PathBuf,
    name: String,
    size: i64,
    modified_ms: i64,
    kind: &'static str,
}

pub fn index_folder(
    folder: &str,
    db: &Path,
    with_thumbs: bool,
    thumbs_dir: Option<&Path>,
) -> Result<CatalogReport, String> {
    let root = std::fs::canonicalize(folder).map_err(|e| format!("Cannot open media folder: {e}"))?;
    if !root.is_dir() {
        return Err("Selected source is not a folder".into());
    }
    let root_text = root.to_string_lossy().to_string();
    let mut conn = catalog::connect(db)?;
    let started = Instant::now();
    let existing = catalog::existing_signatures(&conn, &root_text)?;

    let mut scanned = 0u64;
    let mut unchanged = 0u64;
    let mut skipped = 0u64;
    let mut changed = 0u64;
    let mut pending: Vec<Walked> = Vec::with_capacity(BATCH);

    let flush = |conn: &mut rusqlite::Connection,
                 pending: &mut Vec<Walked>,
                 root_text: &str,
                 changed: &mut u64,
                 skipped: &mut u64| {
        if pending.is_empty() {
            return;
        }
        let take: Vec<Walked> = pending.drain(..).collect();
        let rows: Vec<MediaRecord> = take
            .par_iter()
            .filter_map(|w| extract_record(w, root_text))
            .collect();
        *skipped += (take.len() - rows.len()) as u64;
        match catalog::upsert_batch(conn, &rows) {
            Ok(n) => *changed += n,
            Err(_) => *skipped += rows.len() as u64,
        }
    };

    for entry in WalkDir::new(&root).follow_links(false) {
        let item = match entry {
            Ok(v) => v,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if !item.file_type().is_file() {
            continue;
        }
        let kind = match extension_kind(item.path().as_path()) {
            Some(v) => v,
            None => continue,
        };
        let meta = match item.metadata() {
            Ok(v) => v,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
            .unwrap_or(0);
        let size = meta.len().min(i64::MAX as u64) as i64;
        let path = item.path().to_path_buf();
        let path_key = path.to_string_lossy().into_owned();
        scanned += 1;
        if let Some((esz, emt)) = existing.get(&path_key) {
            if *esz == size && *emt == modified_ms {
                unchanged += 1;
                continue;
            }
        }
        pending.push(Walked {
            name: item.file_name().to_string_lossy().into_owned(),
            path,
            size,
            modified_ms,
            kind,
        });
        if pending.len() >= BATCH {
            flush(&mut conn, &mut pending, &root_text, &mut changed, &mut skipped);
        }
    }
    flush(&mut conn, &mut pending, &root_text, &mut changed, &mut skipped);

    if with_thumbs {
        let cache = thumbs_dir
            .map(Path::to_path_buf)
            .unwrap_or_else(|| thumbs::cache_dir_next_to(db));
        // Thumbnail failures are reported by generate_thumbnails; don't fail the index.
        let _ = thumbs::generate_thumbnails(folder, db, &cache);
    }

    Ok(CatalogReport {
        root: root_text,
        scanned,
        changed,
        unchanged,
        skipped,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

fn extract_record(w: &Walked, root_text: &str) -> Option<MediaRecord> {
    let name = &w.name;
    let fm = exifmeta::read_file_meta(&w.path, w.kind, name, w.modified_ms);
    let media_type = classify_media_type(
        w.kind,
        name,
        fm.camera_make.as_deref(),
        fm.camera_model.as_deref(),
        fm.lens.as_deref(),
        fm.width,
        fm.height,
        fm.duration_ms,
        fm.front_camera,
    )
    .to_string();
    // Cheap phash: only from an embedded EXIF thumbnail so the metadata pass
    // does not decode full-resolution pixels. The thumbs pass fills gaps.
    let phash = fm
        .exif_thumb
        .as_deref()
        .and_then(thumbs::dhash64_from_jpeg_bytes);
    let path_s = w.path.to_string_lossy().into_owned();
    Some(MediaRecord {
        item_id: item_id_for(&path_s),
        path: path_s,
        root: root_text.to_string(),
        name: name.clone(),
        size: w.size,
        modified_ms: w.modified_ms,
        kind: w.kind.to_string(),
        indexed_ms: catalog::now_ms(),
        captured_ms: fm.captured_ms,
        date_source: if fm.date_source.is_empty() {
            "mtime".into()
        } else {
            fm.date_source
        },
        date_confidence: fm.date_confidence,
        lat: fm.lat,
        lon: fm.lon,
        camera_make: fm.camera_make,
        camera_model: fm.camera_model,
        lens: fm.lens,
        width: fm.width,
        height: fm.height,
        duration_ms: fm.duration_ms,
        media_type,
        phash,
        thumb_path: None,
        front_camera: fm.front_camera,
        group_id: None,
        pair_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn accepts_raw_and_movies() {
        assert_eq!(extension_kind(Path::new("camera.ARW")), Some("image"));
        assert_eq!(extension_kind(Path::new("video.mov")), Some("video"));
        assert_eq!(extension_kind(Path::new("notes.txt")), None);
    }

    #[test]
    fn scan_folder_is_uncapped() {
        let base = std::env::temp_dir().join(format!(
            "mnemosyne-scan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();
        for i in 0..120 {
            std::fs::write(base.join(format!("{i:04}.jpg")), []).unwrap();
        }
        let report = scan_folder(base.to_str().unwrap()).unwrap();
        assert_eq!(report.count, 120);
        assert!(!report.truncated);
        let _ = std::fs::remove_dir_all(&base);
    }
}
