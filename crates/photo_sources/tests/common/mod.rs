#![allow(dead_code)]
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::FileOptions;
use zip::ZipWriter;

pub fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mnemosyne-src-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn write_bytes(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, bytes).unwrap();
}

pub fn write_zip(path: &Path, files: &[(&str, &[u8])]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let file = File::create(path).unwrap();
    let mut zip = ZipWriter::new(file);
    let opts = FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in files {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

/// Realistic Takeout sidecar body. Tests must not require us to parse it.
pub const TAKEOUT_JSON: &str = r#"{
  "title": "IMG_0001.jpg",
  "photoTakenTime": { "timestamp": "1704067200", "formatted": "Jan 1, 2024, 12:00:00 AM UTC" },
  "geoData": { "latitude": 37.7749, "longitude": -122.4194, "altitude": 10.0, "latitudeSpan": 0.0, "longitudeSpan": 0.0 }
}"#;

pub const INSTAGRAM_POSTS: &str = r#"[{
  "media": [{
    "uri": "media/posts/202401/IMG_ig.jpg",
    "creation_timestamp": 1704067200,
    "title": "hello"
  }]
}]"#;

pub const FACEBOOK_PHOTO: &str = r#"{
  "photos": [{
    "uri": "photos_and_videos/your_photos/FB_1.jpg",
    "creation_timestamp": 1704067200,
    "media_metadata": { "photo_metadata": { "exif_data": [] } }
  }]
}"#;

pub const SNAP_MEMORIES: &str = r#"{
  "Saved Media": [{
    "Date": "2024-01-01 12:00:00 UTC",
    "Media Type": "Image",
    "Download Link": "memories/mid1.jpg"
  }]
}"#;

pub const TIKTOK_USER: &str = r#"{
  "Video": { "Videos": { "VideoList": [{ "Date": "2024-01-01 12:00:00", "Link": "unused", "Likes": "0" }] } }
}"#;
