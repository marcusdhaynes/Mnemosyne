//! Fast on-disk thumbnail cache. Prefers an embedded EXIF JPEG thumbnail
//! (the big speed win). Falls back to a native decode + 256px JPEG. Also
//! fills in a 64-bit dHash when the indexer could not get one from EXIF.
//! Originals are never written.

use crate::exifmeta::extract_exif_jpeg_thumbnail;
use image::{imageops, DynamicImage, GenericImageView};
use rayon::prelude::*;
use rusqlite::{params, Connection};
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub const THUMB_EDGE: u32 = 256;

#[derive(Debug, serde::Serialize)]
pub struct ThumbReport {
    pub root: String,
    pub considered: u64,
    pub written: u64,
    pub from_exif: u64,
    pub decoded: u64,
    pub skipped: u64,
    pub failed: u64,
    pub elapsed_ms: u128,
}

#[derive(Default)]
struct Outcome {
    path: String,
    dest: Option<PathBuf>,
    phash: Option<u64>,
    from_exif: bool,
    decoded: bool,
    skipped: bool,
    failed: bool,
    written: bool,
}

#[derive(Clone)]
struct ThumbJob {
    path: String,
    #[allow(dead_code)]
    item_id: String,
    kind: String,
    dest: PathBuf,
    already: bool,
    need_phash: bool,
}

pub fn cache_dir_next_to(db: &Path) -> PathBuf {
    db.parent().unwrap_or_else(|| Path::new(".")).join("thumbs")
}

pub fn thumb_path_for(cache: &Path, item_id: &str) -> PathBuf {
    let a = item_id.get(0..2).unwrap_or("00");
    let b = item_id.get(2..4).unwrap_or("00");
    cache.join(a).join(b).join(format!("{item_id}.jpg"))
}

/// 64-bit difference hash (8×8 comparisons on a 9×8 luma image).
pub fn dhash64(img: &DynamicImage) -> u64 {
    let small = imageops::resize(img, 9, 8, imageops::FilterType::Triangle);
    let gray = DynamicImage::ImageRgba8(small).to_luma8();
    let mut hash = 0u64;
    let mut bit = 0;
    for y in 0..8 {
        for x in 0..8 {
            let left = gray.get_pixel(x, y)[0];
            let right = gray.get_pixel(x + 1, y)[0];
            if left < right {
                hash |= 1 << bit;
            }
            bit += 1;
        }
    }
    hash
}

pub fn dhash64_from_jpeg_bytes(bytes: &[u8]) -> Option<u64> {
    let img = image::load_from_memory(bytes).ok()?;
    Some(dhash64(&img))
}

fn load_exif_thumb(path: &Path) -> Option<Vec<u8>> {
    let mut f = File::open(path).ok()?;
    let mut buf = vec![0u8; 128 * 1024];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    extract_exif_jpeg_thumbnail(&buf)
}

fn decode_full_thumb(path: &Path) -> Option<(Vec<u8>, u64)> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let img = image::ImageReader::new(reader)
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let hash = dhash64(&img);
    let jpeg = encode_thumb_jpeg(&img)?;
    Some((jpeg, hash))
}

fn encode_thumb_jpeg(img: &DynamicImage) -> Option<Vec<u8>> {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let resized = if w.max(h) > THUMB_EDGE {
        let scale = THUMB_EDGE as f32 / w.max(h) as f32;
        let nw = ((w as f32) * scale).round().max(1.0) as u32;
        let nh = ((h as f32) * scale).round().max(1.0) as u32;
        img.resize_exact(nw, nh, imageops::FilterType::Triangle)
    } else {
        img.clone()
    };
    let rgb = resized.to_rgb8();
    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut out);
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, 70);
    enc.encode(
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        image::ExtendedColorType::Rgb8,
    )
    .ok()?;
    Some(out)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("jpg.part");
    {
        let mut f = File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(bytes).map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Generate (or reuse) thumbnails for every cataloged item under `folder`.
/// Incremental: existing cache files are skipped unless a phash is still missing.
pub fn generate_thumbnails(folder: &str, db: &Path, cache: &Path) -> Result<ThumbReport, String> {
    let root = fs::canonicalize(folder).map_err(|e| format!("Cannot open media folder: {e}"))?;
    let root_text = root.to_string_lossy().to_string();
    let conn = Connection::open(db).map_err(|e| e.to_string())?;
    conn.busy_timeout(std::time::Duration::from_secs(8))
        .map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT path, item_id, kind, thumb_path, phash FROM indexed_media WHERE root=?1",
        )
        .map_err(|e| e.to_string())?;
    let jobs: Vec<ThumbJob> = stmt
        .query_map(params![root_text], |r| {
            let path: String = r.get(0)?;
            let item_id: String = r.get(1)?;
            let kind: String = r.get(2)?;
            let thumb_path: Option<String> = r.get(3)?;
            let phash: Option<i64> = r.get(4)?;
            let dest = thumb_path_for(cache, &item_id);
            let already = thumb_path
                .as_ref()
                .map(|p| Path::new(p).is_file())
                .unwrap_or(false)
                || dest.is_file();
            Ok(ThumbJob {
                path,
                item_id,
                kind,
                dest,
                already,
                need_phash: phash.is_none(),
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let started = Instant::now();
    let considered = jobs.len() as u64;

    let outcomes: Vec<Outcome> = jobs.into_par_iter().map(process_job).collect();

    let mut written = 0u64;
    let mut from_exif = 0u64;
    let mut decoded = 0u64;
    let mut skipped = 0u64;
    let mut failed = 0u64;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    {
        let mut upd = tx
            .prepare(
                "UPDATE indexed_media SET thumb_path=COALESCE(?2, thumb_path),
                   phash=COALESCE(?3, phash) WHERE path=?1",
            )
            .map_err(|e| e.to_string())?;
        for o in &outcomes {
            if o.written {
                written += 1;
            }
            if o.from_exif {
                from_exif += 1;
            }
            if o.decoded {
                decoded += 1;
            }
            if o.skipped {
                skipped += 1;
            }
            if o.failed {
                failed += 1;
            }
            if o.dest.is_some() || o.phash.is_some() {
                let dest_s = o.dest.as_ref().map(|p| p.to_string_lossy().into_owned());
                let ph = o.phash.map(|h| h as i64);
                let _ = upd.execute(params![o.path, dest_s, ph]);
            }
        }
    }
    tx.commit().map_err(|e| e.to_string())?;

    Ok(ThumbReport {
        root: root_text,
        considered,
        written,
        from_exif,
        decoded,
        skipped,
        failed,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

fn process_job(job: ThumbJob) -> Outcome {
    if job.already && !job.need_phash {
        return Outcome {
            path: job.path,
            dest: Some(job.dest),
            skipped: true,
            ..Outcome::default()
        };
    }
    if job.kind == "video" {
        return video_thumb(&job);
    }
    if let Some(bytes) = load_exif_thumb(Path::new(&job.path)) {
        let hash = dhash64_from_jpeg_bytes(&bytes);
        if !job.already && write_atomic(&job.dest, &bytes).is_err() {
            return Outcome {
                path: job.path,
                failed: true,
                ..Outcome::default()
            };
        }
        // If the cache already exists, still record the phash.
        if job.already && hash.is_some() {
            return Outcome {
                path: job.path,
                dest: Some(job.dest),
                phash: hash,
                from_exif: true,
                skipped: true,
                ..Outcome::default()
            };
        }
        return Outcome {
            path: job.path,
            dest: Some(job.dest),
            phash: hash,
            from_exif: true,
            written: !job.already,
            skipped: job.already,
            ..Outcome::default()
        };
    }
    match decode_full_thumb(Path::new(&job.path)) {
        Some((bytes, hash)) => {
            if !job.already && write_atomic(&job.dest, &bytes).is_err() {
                return Outcome {
                    path: job.path,
                    failed: true,
                    ..Outcome::default()
                };
            }
            Outcome {
                path: job.path,
                dest: Some(job.dest),
                phash: Some(hash),
                decoded: !job.already,
                written: !job.already,
                skipped: job.already,
                ..Outcome::default()
            }
        }
        None => Outcome {
            path: job.path,
            failed: true,
            ..Outcome::default()
        },
    }
}

fn video_thumb(job: &ThumbJob) -> Outcome {
    if job.already {
        return Outcome {
            path: job.path.clone(),
            dest: Some(job.dest.clone()),
            skipped: true,
            ..Outcome::default()
        };
    }
    if let Some(parent) = job.dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    // One keyframe, 256px wide. Skipped silently if ffmpeg is missing.
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-ss",
            "0",
            "-i",
            &job.path,
            "-frames:v",
            "1",
            "-vf",
            "scale='min(256,iw)':-1",
            "-q:v",
            "5",
            "-y",
        ])
        .arg(&job.dest)
        .status();
    match status {
        Ok(s) if s.success() && job.dest.is_file() => Outcome {
            path: job.path.clone(),
            dest: Some(job.dest.clone()),
            written: true,
            decoded: true,
            ..Outcome::default()
        },
        _ => Outcome {
            path: job.path.clone(),
            failed: true,
            ..Outcome::default()
        },
    }
}

/// Compute a dHash for a still image, preferring the EXIF thumbnail.
pub fn phash_for_image(path: &Path, exif_thumb: Option<&[u8]>) -> Option<u64> {
    if let Some(bytes) = exif_thumb {
        if let Some(h) = dhash64_from_jpeg_bytes(bytes) {
            return Some(h);
        }
    }
    if let Some(bytes) = load_exif_thumb(path) {
        return dhash64_from_jpeg_bytes(&bytes);
    }
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let img = image::ImageReader::new(reader)
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    Some(dhash64(&img))
}

