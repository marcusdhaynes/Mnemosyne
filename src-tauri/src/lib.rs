//! Mnemosyne native shell. The web frontend has functional offline import now;
//! native disk-backed indexing is an independently testable engine to connect in a later iteration.
pub mod indexer;
pub mod catalog;
pub mod exifmeta;
pub mod media;
pub mod preview;
pub mod thumbs;
mod commands_index;
use tauri::Manager;
use commands_index::{
    build_preview, catalog_indexed, catalog_stats, generate_thumbnails, write_preview,
};

#[tauri::command]
fn engine_version() -> &'static str { "mnemosyne-indexer-0.2.0" }

#[tauri::command]
async fn inspect_directory(path: String) -> Result<indexer::ScanReport,String> {
  tauri::async_runtime::spawn_blocking(move || indexer::scan_folder(&path))
    .await.map_err(|e|format!("Indexer task failed: {e}"))?
}

fn catalog_location(handle: &tauri::AppHandle) -> Result<std::path::PathBuf,String>{
  Ok(handle.path().app_data_dir().map_err(|e|e.to_string())?.join("index.sqlite3"))
}
/// Indexes metadata only; the originals remain in place. Directory permission
/// scoping should be obtained through the OS picker in the native UI.
#[tauri::command]
async fn index_directory(path: String, app: tauri::AppHandle) -> Result<catalog::CatalogReport,String> {
  let location=catalog_location(&app)?;
  tauri::async_runtime::spawn_blocking(move||catalog::index_folder(&path,&location))
    .await.map_err(|e|e.to_string())?
}
#[tauri::command]
async fn catalog_page(path: String, offset: u64, limit: u32, app: tauri::AppHandle) -> Result<catalog::CatalogPage,String>{
  let location=catalog_location(&app)?;
  tauri::async_runtime::spawn_blocking(move||catalog::page(&path,&location,offset,limit))
    .await.map_err(|e|e.to_string())?
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      engine_version,
      inspect_directory,
      index_directory,
      catalog_page,
      generate_thumbnails,
      build_preview,
      write_preview,
      catalog_stats,
      catalog_indexed
    ])
    .run(tauri::generate_context!())
    .expect("failed to run Mnemosyne");
}
