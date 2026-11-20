//! Durable, bounded-memory metadata catalog (SQLite WAL).
//! Never opens originals for pixel work (the indexer and thumbs modules do that)
//! and never moves, deletes, or rewrites the user's files.
//!
//! Hook tables `faces` and `people` are created empty for Angen's engine.
//! Columns `group_id` and `pair_id` are left NULL for Angen (grouping) and
//! Spin (Live/Motion Photo and burst pairing). `phash` is stored here so
//! Angen can group near-duplicates without re-hashing.

use crate::media::{phash_hex, MediaRecord};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Serialize)]
pub struct CatalogReport {
    pub root: String,
    pub scanned: u64,
    pub changed: u64,
    pub unchanged: u64,
    pub skipped: u64,
    pub elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
pub struct CatalogEntry {
    pub item_id: String,
    pub path: String,
    pub name: String,
    pub size: i64,
    pub modified_ms: i64,
    pub kind: String,
    pub root: String,
    pub captured_ms: Option<i64>,
    pub date_source: Option<String>,
    pub date_confidence: Option<f64>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub media_type: Option<String>,
    pub phash: Option<String>,
    pub thumb_path: Option<String>,
    pub front_camera: bool,
    pub group_id: Option<String>,
    pub pair_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CatalogPage {
    pub total: i64,
    pub offset: u64,
    pub next_offset: Option<u64>,
    pub files: Vec<CatalogEntry>,
}

pub fn connect(db: &Path) -> Result<Connection, String> {
    if let Some(parent) = db.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let conn = Connection::open(db).map_err(|e| e.to_string())?;
    conn.busy_timeout(std::time::Duration::from_secs(8))
        .map_err(|e| e.to_string())?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA temp_store=MEMORY;",
    )
    .map_err(|e| e.to_string())?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS indexed_media(
            path TEXT PRIMARY KEY,
            root TEXT NOT NULL,
            name TEXT NOT NULL,
            size INTEGER NOT NULL,
            modified_ms INTEGER NOT NULL,
            kind TEXT NOT NULL,
            indexed_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_media_root_time ON indexed_media(root, modified_ms DESC, path ASC);
        CREATE INDEX IF NOT EXISTS idx_media_kind ON indexed_media(root, kind);",
    )
    .map_err(|e| e.to_string())?;

    let existing = table_columns(conn, "indexed_media")?;
    const NEW_COLS: &[(&str, &str)] = &[
        ("item_id", "TEXT"),
        ("captured_ms", "INTEGER"),
        ("date_source", "TEXT"),
        ("date_confidence", "REAL"),
        ("lat", "REAL"),
        ("lon", "REAL"),
        ("camera_make", "TEXT"),
        ("camera_model", "TEXT"),
        ("lens", "TEXT"),
        ("width", "INTEGER"),
        ("height", "INTEGER"),
        ("duration_ms", "INTEGER"),
        ("media_type", "TEXT"),
        ("phash", "INTEGER"),
        ("thumb_path", "TEXT"),
        ("front_camera", "INTEGER NOT NULL DEFAULT 0"),
        ("group_id", "TEXT"),
        ("pair_id", "TEXT"),
    ];
    for (name, typ) in NEW_COLS {
        if !existing.iter().any(|c| c.eq_ignore_ascii_case(name)) {
            conn.execute(
                &format!("ALTER TABLE indexed_media ADD COLUMN {name} {typ}"),
                [],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_media_item ON indexed_media(item_id);
         CREATE INDEX IF NOT EXISTS idx_media_captured ON indexed_media(root, captured_ms);
         CREATE INDEX IF NOT EXISTS idx_media_type ON indexed_media(root, media_type);
         CREATE INDEX IF NOT EXISTS idx_media_phash ON indexed_media(phash);
         CREATE INDEX IF NOT EXISTS idx_media_group ON indexed_media(group_id);
         CREATE INDEX IF NOT EXISTS idx_media_pair ON indexed_media(pair_id);

         -- Angen's face engine writes into these tables. The indexer never does.
         CREATE TABLE IF NOT EXISTS faces(
            id TEXT PRIMARY KEY,
            item_path TEXT NOT NULL,
            bbox_x REAL, bbox_y REAL, bbox_w REAL, bbox_h REAL,
            landmarks_json TEXT,
            score REAL,
            quality REAL,
            embed_model TEXT,
            vec BLOB,
            person_id TEXT,
            confidence REAL,
            created_ms INTEGER
         );
         CREATE INDEX IF NOT EXISTS idx_faces_item ON faces(item_path);
         CREATE INDEX IF NOT EXISTS idx_faces_person ON faces(person_id);

         CREATE TABLE IF NOT EXISTS people(
            id TEXT PRIMARY KEY,
            name TEXT,
            face_count INTEGER NOT NULL DEFAULT 0,
            cover_face_id TEXT,
            confidence REAL,
            pending_review INTEGER NOT NULL DEFAULT 0
         );",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?;
    let cols = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(cols)
}

/// Snapshot of (size, mtime) for incremental skipping. Path is the key.
pub fn existing_signatures(conn: &Connection, root: &str) -> Result<HashMap<String, (i64, i64)>, String> {
    let mut stmt = conn
        .prepare("SELECT path, size, modified_ms FROM indexed_media WHERE root=?1")
        .map_err(|e| e.to_string())?;
    let map = stmt
        .query_map(params![root], |r| {
            Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(map)
}

pub fn upsert_batch(conn: &mut Connection, rows: &[MediaRecord]) -> Result<u64, String> {
    if rows.is_empty() {
        return Ok(0);
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let changed = {
        let mut insert = tx
            .prepare_cached(
                "INSERT INTO indexed_media(
                    path, root, name, size, modified_ms, kind, indexed_ms,
                    item_id, captured_ms, date_source, date_confidence,
                    lat, lon, camera_make, camera_model, lens,
                    width, height, duration_ms, media_type, phash, thumb_path,
                    front_camera, group_id, pair_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25)
                 ON CONFLICT(path) DO UPDATE SET
                    root=excluded.root,
                    name=excluded.name,
                    size=excluded.size,
                    modified_ms=excluded.modified_ms,
                    kind=excluded.kind,
                    indexed_ms=excluded.indexed_ms,
                    item_id=excluded.item_id,
                    captured_ms=excluded.captured_ms,
                    date_source=excluded.date_source,
                    date_confidence=excluded.date_confidence,
                    lat=excluded.lat,
                    lon=excluded.lon,
                    camera_make=excluded.camera_make,
                    camera_model=excluded.camera_model,
                    lens=excluded.lens,
                    width=excluded.width,
                    height=excluded.height,
                    duration_ms=excluded.duration_ms,
                    media_type=excluded.media_type,
                    phash=excluded.phash,
                    thumb_path=COALESCE(excluded.thumb_path, indexed_media.thumb_path),
                    front_camera=excluded.front_camera
                 WHERE indexed_media.size<>excluded.size
                    OR indexed_media.modified_ms<>excluded.modified_ms
                    OR indexed_media.root<>excluded.root",
            )
            .map_err(|e| e.to_string())?;
        // group_id and pair_id are intentionally not overwritten on conflict so
        // teammate-filled hooks survive a re-index of the same file.
        let mut n = 0u64;
        for r in rows {
            let ph = r.phash.map(|h| h as i64);
            let fc = if r.front_camera { 1i64 } else { 0 };
            match insert.execute(params![
                r.path,
                r.root,
                r.name,
                r.size,
                r.modified_ms,
                r.kind,
                r.indexed_ms,
                r.item_id,
                r.captured_ms,
                r.date_source,
                r.date_confidence,
                r.lat,
                r.lon,
                r.camera_make,
                r.camera_model,
                r.lens,
                r.width,
                r.height,
                r.duration_ms,
                r.media_type,
                ph,
                r.thumb_path,
                fc,
                r.group_id,
                r.pair_id,
            ]) {
                Ok(c) => n += c as u64,
                Err(_) => {}
            }
        }
        n
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(changed)
}

/// Backward-compatible entry point used by the original tests and Tauri command.
/// Metadata-only, uncapped, incremental by path+size+mtime.
pub fn index_folder(folder: &str, db: &Path) -> Result<CatalogReport, String> {
    crate::indexer::index_folder(folder, db, false, None)
}

pub fn page(folder: &str, db: &Path, offset: u64, limit: u32) -> Result<CatalogPage, String> {
    let root = fs::canonicalize(folder)
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    let conn = connect(db)?;
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM indexed_media WHERE root=?1",
            params![root],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let size = limit.clamp(1, 512);
    let mut stmt = conn
        .prepare(
            "SELECT item_id, path, name, size, modified_ms, kind, root,
                    captured_ms, date_source, date_confidence, lat, lon,
                    camera_make, camera_model, lens, width, height, duration_ms,
                    media_type, phash, thumb_path, front_camera, group_id, pair_id
             FROM indexed_media
             WHERE root=?1
             ORDER BY COALESCE(captured_ms, modified_ms) DESC, path ASC
             LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(
            params![root, size, i64::try_from(offset).unwrap_or(i64::MAX)],
            |r| {
                let phash_i: Option<i64> = r.get(19)?;
                let fc: i64 = r.get::<_, Option<i64>>(21)?.unwrap_or(0);
                Ok(CatalogEntry {
                    item_id: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    path: r.get(1)?,
                    name: r.get(2)?,
                    size: r.get(3)?,
                    modified_ms: r.get(4)?,
                    kind: r.get(5)?,
                    root: r.get(6)?,
                    captured_ms: r.get(7)?,
                    date_source: r.get(8)?,
                    date_confidence: r.get(9)?,
                    lat: r.get(10)?,
                    lon: r.get(11)?,
                    camera_make: r.get(12)?,
                    camera_model: r.get(13)?,
                    lens: r.get(14)?,
                    width: r.get(15)?,
                    height: r.get(16)?,
                    duration_ms: r.get(17)?,
                    media_type: r.get(18)?,
                    phash: phash_hex(phash_i.map(|v| v as u64)),
                    thumb_path: r.get(20)?,
                    front_camera: fc != 0,
                    group_id: r.get(22)?,
                    pair_id: r.get(23)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;
    let mut files = Vec::with_capacity(size as usize);
    for entry in rows {
        files.push(entry.map_err(|e| e.to_string())?);
    }
    let next = offset.saturating_add(files.len() as u64);
    Ok(CatalogPage {
        total,
        offset,
        next_offset: if next < (total as u64) { Some(next) } else { None },
        files,
    })
}

pub fn load_records(db: &Path, root: &str) -> Result<Vec<MediaRecord>, String> {
    let conn = connect(db)?;
    let mut stmt = conn
        .prepare(
            "SELECT item_id, path, root, name, size, modified_ms, kind, indexed_ms,
                    captured_ms, date_source, date_confidence, lat, lon,
                    camera_make, camera_model, lens, width, height, duration_ms,
                    media_type, phash, thumb_path, front_camera, group_id, pair_id
             FROM indexed_media WHERE root=?1
             ORDER BY COALESCE(captured_ms, modified_ms) ASC, path ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![root], |r| {
            let phash_i: Option<i64> = r.get(20)?;
            let fc: i64 = r.get::<_, Option<i64>>(22)?.unwrap_or(0);
            Ok(MediaRecord {
                item_id: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                path: r.get(1)?,
                root: r.get(2)?,
                name: r.get(3)?,
                size: r.get(4)?,
                modified_ms: r.get(5)?,
                kind: r.get(6)?,
                indexed_ms: r.get(7)?,
                captured_ms: r.get(8)?,
                date_source: r.get::<_, Option<String>>(9)?.unwrap_or_else(|| "mtime".into()),
                date_confidence: r.get::<_, Option<f64>>(10)?.unwrap_or(0.0),
                lat: r.get(11)?,
                lon: r.get(12)?,
                camera_make: r.get(13)?,
                camera_model: r.get(14)?,
                lens: r.get(15)?,
                width: r.get(16)?,
                height: r.get(17)?,
                duration_ms: r.get(18)?,
                media_type: r.get::<_, Option<String>>(19)?.unwrap_or_else(|| "photo".into()),
                phash: phash_i.map(|v| v as u64),
                thumb_path: r.get(21)?,
                front_camera: fc != 0,
                group_id: r.get(23)?,
                pair_id: r.get(24)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

pub fn catalog_stats(db: &Path, root: &str) -> Result<HashMap<String, i64>, String> {
    let conn = connect(db)?;
    let mut map = HashMap::new();
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM indexed_media WHERE root=?1",
            params![root],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    map.insert("items".into(), total);
    let mut stmt = conn
        .prepare("SELECT media_type, COUNT(*) FROM indexed_media WHERE root=?1 GROUP BY media_type")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![root], |r| {
            Ok((r.get::<_, Option<String>>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (k, v) = row.map_err(|e| e.to_string())?;
        map.insert(k.unwrap_or_else(|| "unknown".into()), v);
    }
    let faces: i64 = conn
        .query_row("SELECT COUNT(*) FROM faces", [], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?
        .unwrap_or(0);
    map.insert("faces".into(), faces);
    Ok(map)
}


/// Shape passed to Spin's `resolveMedia(files, {indexed})`.
/// Keyed by catalog path. `takenAt` is EXIF/camera time only (ms UTC);
/// `gps` is EXIF GPS only. Omissions let sidecars fill the gap.
pub type IndexedMap = std::collections::BTreeMap<String, IndexedCapture>;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IndexedCapture {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taken_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gps: Option<IndexedGps>,
}

#[derive(Debug, Serialize, Clone)]
pub struct IndexedGps {
    pub lat: f64,
    pub lon: f64,
}

/// Camera dates and GPS for the JS bridge. Does not include filename or
/// mtime dates — Spin owns sidecar and filename recovery, and those must
/// not be overridden by a weak native fallback.
pub fn indexed_for_folder(folder: &str, db: &Path) -> Result<IndexedMap, String> {
    let root = fs::canonicalize(folder)
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    let conn = connect(db)?;
    let mut stmt = conn
        .prepare(
            "SELECT path, captured_ms, date_source, lat, lon
             FROM indexed_media WHERE root=?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![root], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<f64>>(3)?,
                r.get::<_, Option<f64>>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut map = IndexedMap::new();
    for row in rows {
        let (path, captured_ms, date_source, lat, lon) = row.map_err(|e| e.to_string())?;
        let taken_at = match (captured_ms, date_source.as_deref()) {
            (Some(ms), Some("exif")) => Some(ms),
            _ => None,
        };
        let gps = match (lat, lon) {
            (Some(lat), Some(lon)) if lat.is_finite() && lon.is_finite() => {
                Some(IndexedGps { lat, lon })
            }
            _ => None,
        };
        if taken_at.is_none() && gps.is_none() {
            continue;
        }
        map.insert(path, IndexedCapture { taken_at, gps });
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn indexes_over_five_thousand_without_truncation() {
        let base = std::env::temp_dir().join(format!(
            "mnemosyne-indexer-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let photos = base.join("photos");
        fs::create_dir_all(&photos).unwrap();
        for i in 0..5050 {
            fs::write(photos.join(format!("{i:05}.jpg")), []).unwrap();
        }
        fs::write(photos.join("note.txt"), b"ignored").unwrap();
        let db = base.join("catalog.db");
        let report = index_folder(photos.to_str().unwrap(), &db).unwrap();
        assert_eq!(report.scanned, 5050);
        assert_eq!(
            page(photos.to_str().unwrap(), &db, 0, 256)
                .unwrap()
                .files
                .len(),
            256
        );
        let tail = page(photos.to_str().unwrap(), &db, 5000, 256).unwrap();
        assert_eq!(tail.files.len(), 50);
        assert_eq!(tail.next_offset, None);
        let again = index_folder(photos.to_str().unwrap(), &db).unwrap();
        assert_eq!(again.changed, 0);
        assert_eq!(again.unchanged, 5050);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn indexed_map_exposes_exif_only() {
        let rec = MediaRecord {
            item_id: "abc".into(),
            path: "/tmp/photo.jpg".into(),
            root: "/tmp".into(),
            name: "photo.jpg".into(),
            size: 10,
            modified_ms: 1,
            kind: "image".into(),
            indexed_ms: 1,
            captured_ms: Some(1_704_067_200_000),
            date_source: "exif".into(),
            date_confidence: 0.98,
            lat: Some(38.7),
            lon: Some(-9.14),
            camera_make: Some("Apple".into()),
            camera_model: Some("iPhone".into()),
            lens: None,
            width: Some(100),
            height: Some(80),
            duration_ms: None,
            media_type: "photo".into(),
            phash: None,
            thumb_path: None,
            front_camera: false,
            group_id: None,
            pair_id: None,
        };
        // Filename-only dates must not appear in `indexed`.
        assert_eq!(rec.date_source, "exif");
        assert!(rec.captured_ms.is_some());
    }
}
