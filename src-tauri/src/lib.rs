//! Mnemosyne native shell. The web frontend has functional offline import now;
//! native disk-backed indexing is an independently testable engine to connect in a later iteration.
mod indexer;
mod catalog;
use std::path::PathBuf;
use tauri::Manager;
use photo_sources::{
    apply as apply_source_commit, discover_devices, dry_run, enumerate_path_default, unpack_export,
    undo as undo_source_commit, CloudKind, CommitPlan,
};
use photo_sources::sources::cloud;

#[tauri::command]
fn engine_version() -> &'static str { "mnemosyne-indexer-0.1.0" }

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

/// Windows drive-letter / DCIM discovery. Extra roots are useful on Linux and in tests.
#[tauri::command]
async fn discover_removable_sources(extra_roots: Option<Vec<String>>) -> Result<Vec<photo_sources::DiscoveredDevice>, String> {
  let roots: Vec<PathBuf> = extra_roots.unwrap_or_default().into_iter().map(PathBuf::from).collect();
  tauri::async_runtime::spawn_blocking(move || discover_devices(&roots).map_err(|e| e.to_string()))
    .await.map_err(|e| e.to_string())?
}

/// Unpack and/or walk a folder, zip export, or mounted device. Applies the pass-through format hook.
#[tauri::command]
async fn enumerate_source(path: String) -> Result<photo_sources::Enumeration, String> {
  tauri::async_runtime::spawn_blocking(move || enumerate_path_default(&path).map_err(|e| e.to_string()))
    .await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn unpack_source_archive(archive: String, dest: String) -> Result<photo_sources::sources::UnpackReport, String> {
  tauri::async_runtime::spawn_blocking(move || unpack_export(PathBuf::from(archive).as_path(), PathBuf::from(dest).as_path()).map_err(|e| e.to_string()))
    .await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn list_cloud_stubs() -> Vec<photo_sources::CloudStub> {
  cloud::list_stubs()
}

#[tauri::command]
fn connect_cloud_source(kind: CloudKind) -> Result<(), String> {
  cloud::connect(kind).map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
fn dry_run_commit(plan: CommitPlan) -> Result<photo_sources::DryRunReport, String> {
  dry_run(&plan).map_err(|e| e.to_string())
}

#[tauri::command]
async fn apply_commit(plan: CommitPlan, journal: String) -> Result<photo_sources::Journal, String> {
  tauri::async_runtime::spawn_blocking(move || apply_source_commit(&plan, PathBuf::from(journal).as_path()).map_err(|e| e.to_string()))
    .await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn revert_commit(journal: String) -> Result<photo_sources::Journal, String> {
  tauri::async_runtime::spawn_blocking(move || undo_source_commit(PathBuf::from(journal).as_path()).map_err(|e| e.to_string()))
    .await.map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      engine_version,
      inspect_directory,
      index_directory,
      catalog_page,
      discover_removable_sources,
      enumerate_source,
      unpack_source_archive,
      list_cloud_stubs,
      connect_cloud_source,
      dry_run_commit,
      apply_commit,
      revert_commit
    ])
    .run(tauri::generate_context!())
    .expect("failed to run Mnemosyne");
}
