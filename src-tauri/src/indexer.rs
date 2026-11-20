use std::{fs, path::Path, time::UNIX_EPOCH};
use serde::Serialize;
use walkdir::WalkDir;

/// Metadata-only scanner; never moves, deletes or changes original files.
/// A future native SQLite catalog can consume these items in bounded batches.
#[derive(Debug,Serialize)]
pub struct PhotoEntry { pub path: String, pub name: String, pub bytes: u64, pub modified_ms: u64, pub kind: &'static str }
#[derive(Debug,Serialize)]
pub struct ScanReport { pub root: String, pub count: usize, pub truncated: bool, pub files: Vec<PhotoEntry> }
fn extension_kind(path: &Path) -> Option<&'static str> {
  match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
    "jpg"|"jpeg"|"png"|"webp"|"gif"|"bmp"|"tif"|"tiff"|"avif"|"heic"|"heif"|"jxl"|"dng"|"nef"|"cr2"|"arw"|"raf"|"orf"|"rw2" => Some("image"),
    "mp4"|"m4v"|"mov"|"webm"|"avi"|"mkv"|"3gp" => Some("video"),
    _ => None
  }
}
pub fn scan_folder(directory:&str)->Result<ScanReport,String>{
  let root=fs::canonicalize(directory).map_err(|e|format!("Folder not available: {e}"))?;
  if !root.is_dir(){return Err("Not a folder".into())}
  let mut files=Vec::new();let mut truncated=false;
  for item in WalkDir::new(&root).follow_links(false).into_iter().filter_map(Result::ok){
    if !item.file_type().is_file(){continue}
    let kind=match extension_kind(item.path()){Some(kind)=>kind,None=>continue};
    let meta=match item.metadata(){Ok(m)=>m,Err(_)=>continue};
    let modified_ms=meta.modified().ok().and_then(|v|v.duration_since(UNIX_EPOCH).ok()).map(|d|d.as_millis() as u64).unwrap_or(0);
    files.push(PhotoEntry{path:item.path().to_string_lossy().into_owned(),name:item.file_name().to_string_lossy().into_owned(),bytes:meta.len(),modified_ms,kind});
    if files.len()==5000{truncated=true;break}
  }
  Ok(ScanReport{root:root.to_string_lossy().into_owned(),count:files.len(),truncated,files})
}
#[cfg(test)]mod tests{use super::*;#[test]fn accepts_raw_and_movies(){assert_eq!(extension_kind(Path::new("camera.ARW")),Some("image"));assert_eq!(extension_kind(Path::new("video.mov")),Some("video"));assert_eq!(extension_kind(Path::new("notes.txt")),None);}}
