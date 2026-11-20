//! Tauri commands for the fast indexer, thumbnails, and pass-1 preview.
//! Kept in this module so `lib.rs` stays a short `mod` + `generate_handler!`
//! list and merges cleanly with Anky's source/commit commands.
//! Command names here do not overlap hers (`discover_removable_sources`,
//! `enumerate_source`, `unpack_source_archive`, `list_cloud_stubs`,
//! `connect_cloud_source`, `dry_run_commit`, `apply_commit`, `revert_commit`).

use crate::{catalog, preview, thumbs};
use tauri::Manager;

fn catalog_location(handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("index.sqlite3"))
}

fn thumbs_location(handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("thumbs"))
}

/// Parallel thumbnail build. Uses embedded EXIF thumbs when present.
/// Incremental: existing cache files are reused. Does not move originals.
#[tauri::command]
pub async fn generate_thumbnails(
    path: String,
    app: tauri::AppHandle,
) -> Result<thumbs::ThumbReport, String> {
    let db = catalog_location(&app)?;
    let cache = thumbs_location(&app)?;
    tauri::async_runtime::spawn_blocking(move || thumbs::generate_thumbnails(&path, &db, &cache))
        .await
        .map_err(|e| e.to_string())?
}

/// Pass-1 preview from the catalog: events, date buckets, type tags, and
/// proposed album assignments. Never touches the original files.
#[tauri::command]
pub async fn build_preview(path: String, app: tauri::AppHandle) -> Result<preview::Preview, String> {
    let db = catalog_location(&app)?;
    tauri::async_runtime::spawn_blocking(move || preview::build_preview(&path, &db))
        .await
        .map_err(|e| e.to_string())?
}

/// Write preview.json for Anky's commit dry-run / Trice's UI fixture.
#[tauri::command]
pub async fn write_preview(
    path: String,
    out: String,
    app: tauri::AppHandle,
) -> Result<preview::Preview, String> {
    let db = catalog_location(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        preview::write_preview_json(&path, &db, std::path::Path::new(&out))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn catalog_stats(
    path: String,
    app: tauri::AppHandle,
) -> Result<std::collections::HashMap<String, i64>, String> {
    let db = catalog_location(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let root = std::fs::canonicalize(&path)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .to_string();
        catalog::catalog_stats(&db, &root)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Camera dates and GPS for Spin's `resolveMedia(..., {indexed})`.
/// Returns an object keyed by file path: `{ takenAt?: msUTC, gps?: {lat, lon} }`.
/// Only EXIF/camera values are included, so sidecars can still fill gaps.
/// Native values win when Spin merges them.
#[tauri::command]
pub async fn catalog_indexed(
    path: String,
    app: tauri::AppHandle,
) -> Result<catalog::IndexedMap, String> {
    let db = catalog_location(&app)?;
    tauri::async_runtime::spawn_blocking(move || catalog::indexed_for_folder(&path, &db))
        .await
        .map_err(|e| e.to_string())?
}
