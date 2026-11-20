use std::path::Path;

/// Extension-only classification used to locate media and sidecar files.
/// Format interpretation (pairing, dates, bursts) belongs to Spin via `FormatResolver`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Media,
    Sidecar,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Image,
    Video,
}

const IMAGE_EXT: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff", "avif", "heic", "heif", "jxl",
    "dng", "nef", "cr2", "cr3", "arw", "raf", "orf", "rw2", "srw", "pef", "x3f", "3fr", "iiq",
    "erf", "mef", "mrw", "kdc", "rwl", "mos",
];
const VIDEO_EXT: &[&str] = &[
    "mp4", "m4v", "mov", "webm", "avi", "mkv", "3gp", "3g2", "mts", "m2ts", "mpeg", "mpg", "wmv",
    "flv", "f4v", "vob", "ogv", "ts", "asf", "hevc", "mxf",
];
const SIDECAR_EXT: &[&str] = &["json", "xmp", "aae", "thm", "xml"];

pub fn ext_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
}

pub fn media_kind(path: &Path) -> Option<MediaKind> {
    match ext_of(path)?.as_str() {
        e if IMAGE_EXT.contains(&e) => Some(MediaKind::Image),
        e if VIDEO_EXT.contains(&e) => Some(MediaKind::Video),
        _ => None,
    }
}

pub fn is_sidecar_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".supplemental-metadata.json") || lower.ends_with(".supplemental-meta.json") {
        return true;
    }
    // Takeout often uses photo.jpg.json; Snapchat uses memories_history.json/html.
    if lower.ends_with(".json") || lower.ends_with(".xmp") || lower.ends_with(".aae") {
        return true;
    }
    if lower.ends_with(".html") && lower.contains("memories") {
        return true;
    }
    if lower == "user_data.json" || lower == "posts_1.json" || lower == "media.json" {
        return true;
    }
    false
}

pub fn role_of(path: &Path) -> Role {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if is_sidecar_name(name) {
        return Role::Sidecar;
    }
    if media_kind(path).is_some() {
        return Role::Media;
    }
    if let Some(ext) = ext_of(path) {
        if SIDECAR_EXT.contains(&ext.as_str()) {
            return Role::Sidecar;
        }
    }
    Role::Other
}

/// Nearby sidecar paths for a media file, by naming convention only. Does not read file contents.
pub fn sidecar_candidates_for(media: &Path, available: &[std::path::PathBuf]) -> Vec<std::path::PathBuf> {
    let Some(name) = media.file_name().and_then(|n| n.to_str()) else {
        return Vec::new();
    };
    let stem = media.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let parent = media.parent();
    let suffixes = [
        format!("{name}.json"),
        format!("{name}.supplemental-metadata.json"),
        format!("{name}.supplemental-meta.json"),
        format!("{stem}.json"),
        format!("{stem}.xmp"),
        format!("{stem}.XMP"),
        format!("{name}.xmp"),
        format!("{stem}.aae"),
    ];
    available
        .iter()
        .filter(|p| {
            let Some(n) = p.file_name().and_then(|n| n.to_str()) else {
                return false;
            };
            if parent.is_some() && p.parent() != parent {
                // Allow same-folder only for name-matched sidecars.
                return false;
            }
            suffixes.iter().any(|s| n.eq_ignore_ascii_case(s))
        })
        .cloned()
        .collect()
}

/// O(1) sidecar lookup by directory + conventional filename suffixes.
pub fn sidecar_candidates_indexed(
    media: &Path,
    index: &std::collections::HashMap<(std::path::PathBuf, String), std::path::PathBuf>,
) -> Vec<std::path::PathBuf> {
    let Some(name) = media.file_name().and_then(|n| n.to_str()) else {
        return Vec::new();
    };
    let stem = media.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let Some(parent) = media.parent() else {
        return Vec::new();
    };
    let parent = parent.to_path_buf();
    let suffixes = [
        format!("{name}.json"),
        format!("{name}.supplemental-metadata.json"),
        format!("{name}.supplemental-meta.json"),
        format!("{stem}.json"),
        format!("{stem}.xmp"),
        format!("{name}.xmp"),
        format!("{stem}.aae"),
    ];
    let mut out = Vec::new();
    for suffix in suffixes {
        if let Some(path) = index.get(&(parent.clone(), suffix.to_ascii_lowercase())) {
            if !out.iter().any(|p| p == path) {
                out.push(path.clone());
            }
        }
    }
    out
}
