//! Mnemosyne native shell. The web frontend has functional offline import now;
//! native disk-backed indexing is an independently testable engine to connect in a later iteration.
mod indexer;

#[tauri::command]
fn engine_version() -> &'static str { "mnemosyne-indexer-0.1.0" }

#[tauri::command]
async fn inspect_directory(path: String) -> Result<indexer::ScanReport,String> {
  tauri::async_runtime::spawn_blocking(move || indexer::scan_folder(&path))
    .await.map_err(|e|format!("Indexer task failed: {e}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![engine_version,inspect_directory])
    .run(tauri::generate_context!())
    .expect("failed to run Mnemosyne");
}
