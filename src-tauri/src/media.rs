//! Shared media types, extension maps, date-from-filename, and type tags.
//! This module never moves, deletes, or rewrites original files.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff", "avif", "heic", "heif",
    "jxl", "dng", "nef", "cr2", "cr3", "arw", "raf", "orf", "rw2", "srw", "pef", "x3f",
    "3fr", "iiq", "erf", "mef", "mrw", "kdc", "rwl", "mos",
];
pub const VIDEO_EXTS: &[&str] = &[
    "mp4", "m4v", "mov", "webm", "avi", "mkv", "3gp", "3g2", "mts", "m2ts", "mpeg",
    "mpg", "wmv", "flv", "f4v", "vob", "ogv", "ts", "asf", "hevc", "mxf",
];

/// Hook columns reserved for teammates. The indexer leaves them NULL.
/// * `group_id` — Angen's near-duplicate / people grouping
/// * `pair_id` — Spin's Live Photo, Motion Photo, and burst pairing
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupHooks {
    pub group_id: Option<String>,
    pub pair_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaRecord {
    pub item_id: String,
    pub path: String,
    pub root: String,
    pub name: String,
    pub size: i64,
    pub modified_ms: i64,
    pub kind: String,
    pub indexed_ms: i64,
    pub captured_ms: Option<i64>,
    pub date_source: String,
    pub date_confidence: f64,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    /// photo | screenshot | selfie | video | short_vertical | screen_recording
    pub media_type: String,
    /// 64-bit dHash, stored so Angen can group near-duplicates later.
    pub phash: Option<u64>,
    pub thumb_path: Option<String>,
    pub front_camera: bool,
    pub group_id: Option<String>,
    pub pair_id: Option<String>,
}

pub fn extension_kind(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if IMAGE_EXTS.iter().any(|e| *e == ext) {
        Some("image")
    } else if VIDEO_EXTS.iter().any(|e| *e == ext) {
        Some("video")
    } else {
        None
    }
}

pub fn item_id_for(path: &str) -> String {
    let hash = blake3::hash(path.as_bytes());
    let mut s = String::with_capacity(32);
    for b in &hash.as_bytes()[..16] {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

pub fn looks_front_camera(parts: &[&str]) -> bool {
    let blob = parts
        .iter()
        .flat_map(|s| s.split_whitespace())
        .map(|s| s.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    let hay = blob.replace('_', " ").replace('-', " ");
    const KEYS: &[&str] = &[
        "front camera",
        "front lens",
        "facetime",
        "selfie",
        "facing front",
        "facing:front",
        "camera facing front",
        "truefront",
        "front-facing",
        "front facing",
    ];
    KEYS.iter().any(|k| hay.contains(k)) || hay.split_whitespace().any(|w| w == "front")
}

fn is_screen_resolution(width: Option<i64>, height: Option<i64>) -> bool {
    let (w, h) = match (width, height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => (w, h),
        _ => return false,
    };
    let (a, b) = if w >= h { (w, h) } else { (h, w) };
    const COMMON: &[(i64, i64)] = &[
        (1920, 1080),
        (2560, 1440),
        (3840, 2160),
        (1280, 720),
        (1366, 768),
        (1440, 900),
        (1536, 864),
        (1600, 900),
        (2560, 1600),
        (2880, 1800),
        (3024, 1964),
        (1170, 2532),
        (1284, 2778),
        (1179, 2556),
        (1290, 2796),
        (1080, 2340),
        (1080, 2400),
        (1080, 1920),
        (720, 1280),
        (750, 1334),
        (1125, 2436),
        (1242, 2688),
        (828, 1792),
        (1224, 2700),
        (1206, 2622),
        (1024, 768),
        (1360, 768),
        (1600, 1200),
        (2048, 1536),
        (2160, 3840),
        (1488, 2266),
        (1640, 2360),
    ];
    COMMON.iter().any(|&(x, y)| a == x && b == y)
}

fn is_portrait_short(width: Option<i64>, height: Option<i64>) -> bool {
    match (width, height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => (h as f64 / w as f64) >= 1.5,
        _ => false,
    }
}

/// Type tags for the fast preview. Screenshot / short / selfie only, no
/// Live Photo or burst pairing (those belong to Spin).
pub fn classify_media_type(
    kind: &str,
    name: &str,
    camera_make: Option<&str>,
    camera_model: Option<&str>,
    lens: Option<&str>,
    width: Option<i64>,
    height: Option<i64>,
    duration_ms: Option<i64>,
    front_camera: bool,
) -> &'static str {
    let name_l = name.to_ascii_lowercase();
    if kind == "video" {
        if name_l.contains("screen recording")
            || name_l.contains("screen-recording")
            || name_l.contains("screenrecording")
            || name_l.contains("screen_recording")
        {
            return "screen_recording";
        }
        let portrait = is_portrait_short(width, height);
        let short_dur = duration_ms.map(|d| d <= 180_000).unwrap_or(false);
        if portrait && (short_dur || duration_ms.is_none()) {
            return "short_vertical";
        }
        if short_dur && portrait {
            return "short_vertical";
        }
        // 9:16-ish clips under three minutes, even if duration is known.
        if portrait && short_dur {
            return "short_vertical";
        }
        if short_dur && !portrait {
            // landscape or square clips ≤ 180s are still "video"; only vertical
            // shorts get the short_vertical tag (aspect + duration).
            return "video";
        }
        return "video";
    }

    if name_l.contains("screenshot") || name_l.contains("screen shot") || name_l.contains("screen-shot")
    {
        return "screenshot";
    }
    if name_l.contains("selfie") || front_camera {
        return "selfie";
    }
    if looks_front_camera(&[
        camera_make.unwrap_or(""),
        camera_model.unwrap_or(""),
        lens.unwrap_or(""),
    ]) {
        return "selfie";
    }

    let no_camera = camera_make.map(|s| s.trim().is_empty()).unwrap_or(true)
        && camera_model.map(|s| s.trim().is_empty()).unwrap_or(true);
    let png = name_l.ends_with(".png");
    let screen = is_screen_resolution(width, height);
    // Require two signals so ordinary PNG photos are not tagged as screenshots.
    let signals = (no_camera as u8) + (png as u8) + (screen as u8);
    if no_camera && (screen || (png && screen) || (png && no_camera && screen)) {
        return "screenshot";
    }
    if signals >= 2 && no_camera {
        return "screenshot";
    }
    "photo"
}

/// Parse a capture date out of a filename. Returns (unix_ms, confidence).
pub fn date_from_filename(name: &str) -> Option<(i64, f64)> {
    let bytes = name.as_bytes();
    // IMG_20240101_142201 / PXL_20240101_142201123 / VID_20240101_142201
    if let Some(ms) = find_compact_datetime(bytes) {
        return Some((ms, 0.85));
    }
    // WhatsApp IMG-20240101-WA0001
    if let Some(ms) = find_whatsapp_date(name) {
        return Some((ms, 0.8));
    }
    // Screenshot_2024-01-01-14-22-01 or Screenshot_20240101-142201
    if let Some(ms) = find_screenshot_date(name) {
        return Some((ms, 0.85));
    }
    // Generic 2024-01-01 or 2024_01_01
    if let Some(ms) = find_dashed_date(name) {
        return Some((ms, 0.6));
    }
    None
}

fn is_digit(b: u8) -> bool {
    b.is_ascii_digit()
}

fn find_compact_datetime(bytes: &[u8]) -> Option<i64> {
    // 8-digit date followed by '_' or '-' and 6-digit time
    let n = bytes.len();
    for i in 0..n.saturating_sub(15) {
        if !(0..8).all(|k| is_digit(bytes[i + k])) {
            continue;
        }
        let sep = bytes[i + 8];
        if sep != b'_' && sep != b'-' {
            continue;
        }
        if !(0..6).all(|k| is_digit(bytes[i + 9 + k])) {
            continue;
        }
        // reject if preceded by a digit (part of a longer number)
        if i > 0 && is_digit(bytes[i - 1]) {
            continue;
        }
        let y: i32 = atoi(&bytes[i..i + 4])?;
        let mo: u32 = atoi(&bytes[i + 4..i + 6])? as u32;
        let d: u32 = atoi(&bytes[i + 6..i + 8])? as u32;
        let h: u32 = atoi(&bytes[i + 9..i + 11])? as u32;
        let mi: u32 = atoi(&bytes[i + 11..i + 13])? as u32;
        let s: u32 = atoi(&bytes[i + 13..i + 15])? as u32;
        if let Some(ms) = ymd_hms_ms(y, mo, d, h, mi, s) {
            return Some(ms);
        }
    }
    None
}

fn find_whatsapp_date(name: &str) -> Option<i64> {
    // IMG-20240101-WA or VID-20240101-WA
    let up = name.to_ascii_uppercase();
    let idx = up.find("-WA")?;
    if idx < 8 {
        return None;
    }
    let date = &up[idx - 8..idx];
    if !date.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let y: i32 = date[0..4].parse().ok()?;
    let mo: u32 = date[4..6].parse().ok()?;
    let d: u32 = date[6..8].parse().ok()?;
    ymd_hms_ms(y, mo, d, 0, 0, 0)
}

fn find_screenshot_date(name: &str) -> Option<i64> {
    let l = name.to_ascii_lowercase();
    if !l.contains("screenshot") && !l.contains("screen shot") {
        return None;
    }
    find_dashed_date(name).or_else(|| {
        // Screenshot_20240101-142201
        find_compact_datetime(name.as_bytes())
    })
}

fn find_dashed_date(name: &str) -> Option<i64> {
    let bytes = name.as_bytes();
    let n = bytes.len();
    for i in 0..n.saturating_sub(10) {
        if !(is_digit(bytes[i])
            && is_digit(bytes[i + 1])
            && is_digit(bytes[i + 2])
            && is_digit(bytes[i + 3])
            && (bytes[i + 4] == b'-' || bytes[i + 4] == b'_')
            && is_digit(bytes[i + 5])
            && is_digit(bytes[i + 6])
            && (bytes[i + 7] == b'-' || bytes[i + 7] == b'_')
            && is_digit(bytes[i + 8])
            && is_digit(bytes[i + 9]))
        {
            continue;
        }
        if i > 0 && is_digit(bytes[i - 1]) {
            continue;
        }
        let y: i32 = atoi(&bytes[i..i + 4])?;
        let mo: u32 = atoi(&bytes[i + 5..i + 7])? as u32;
        let d: u32 = atoi(&bytes[i + 8..i + 10])? as u32;
        let mut h = 0u32;
        let mut mi = 0u32;
        let mut s = 0u32;
        // optional _HH-MM-SS or -HH-MM-SS after the date
        if i + 16 <= n && (bytes[i + 10] == b'-' || bytes[i + 10] == b'_' || bytes[i + 10] == b' ') {
            if (0..2).all(|k| is_digit(bytes[i + 11 + k]))
                && (bytes[i + 13] == b'-' || bytes[i + 13] == b'_')
                && (0..2).all(|k| is_digit(bytes[i + 14 + k]))
            {
                h = atoi(&bytes[i + 11..i + 13])? as u32;
                mi = atoi(&bytes[i + 14..i + 16])? as u32;
                if i + 19 <= n
                    && (bytes[i + 16] == b'-' || bytes[i + 16] == b'_')
                    && (0..2).all(|k| is_digit(bytes[i + 17 + k]))
                {
                    s = atoi(&bytes[i + 17..i + 19])? as u32;
                }
            }
        }
        if let Some(ms) = ymd_hms_ms(y, mo, d, h, mi, s) {
            return Some(ms);
        }
    }
    None
}

fn atoi(s: &[u8]) -> Option<i32> {
    let text = std::str::from_utf8(s).ok()?;
    text.parse().ok()
}

/// Civil date-time to Unix milliseconds (naive, treated as UTC).
pub fn ymd_hms_ms(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> Option<i64> {
    if !(1..=12).contains(&mo) || d == 0 || d > 31 || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    if y < 1970 || y > 2100 {
        return None;
    }
    let dim = days_in_month(y, mo)?;
    if d > dim {
        return None;
    }
    let mut days: i64 = 0;
    for year in 1970..y {
        days += if is_leap(year) { 366 } else { 365 };
    }
    for m in 1..mo {
        days += days_in_month(y, m)? as i64;
    }
    days += (d as i64) - 1;
    Some(days * 86_400_000 + ((h as i64 * 60 + mi as i64) * 60 + s as i64) * 1000)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

fn days_in_month(y: i32, mo: u32) -> Option<u32> {
    Some(match mo {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => return None,
    })
}

pub fn apply_offset_ms(base_ms: i64, offset: &str) -> i64 {
    // "+05:30" or "-0700" or "Z"
    let t = offset.trim();
    if t.is_empty() || t == "Z" || t == "z" {
        return base_ms;
    }
    let sign = if t.starts_with('-') { -1i64 } else { 1i64 };
    let rest = t.trim_start_matches(['+', '-']);
    let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 2 {
        return base_ms;
    }
    let hh: i64 = digits.get(0..2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let mm: i64 = if digits.len() >= 4 {
        digits.get(2..4).and_then(|s| s.parse().ok()).unwrap_or(0)
    } else {
        0
    };
    base_ms - sign * (hh * 3_600_000 + mm * 60_000)
}

pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    2.0 * R * a.sqrt().asin()
}

pub fn phash_hex(phash: Option<u64>) -> Option<String> {
    phash.map(|h| format!("{h:016x}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn epoch_and_known_date() {
        assert_eq!(ymd_hms_ms(1970, 1, 1, 0, 0, 0), Some(0));
        assert_eq!(ymd_hms_ms(2024, 1, 1, 0, 0, 0), Some(1_704_067_200_000));
        assert_eq!(ymd_hms_ms(2024, 2, 29, 12, 0, 0).is_some(), true);
        assert_eq!(ymd_hms_ms(2023, 2, 29, 0, 0, 0), None);
    }

    #[test]
    fn filename_dates() {
        let (ms, _) = date_from_filename("IMG_20240101_142201.jpg").unwrap();
        assert_eq!(ms, ymd_hms_ms(2024, 1, 1, 14, 22, 1).unwrap());
        let (ms, _) = date_from_filename("IMG-20240615-WA0003.jpg").unwrap();
        assert_eq!(ms, ymd_hms_ms(2024, 6, 15, 0, 0, 0).unwrap());
        let (ms, _) = date_from_filename("Screenshot_2024-03-08-09-30-00.png").unwrap();
        assert_eq!(ms, ymd_hms_ms(2024, 3, 8, 9, 30, 0).unwrap());
    }

    #[test]
    fn kinds_and_types() {
        assert_eq!(extension_kind(Path::new("camera.ARW")), Some("image"));
        assert_eq!(extension_kind(Path::new("video.mov")), Some("video"));
        assert_eq!(extension_kind(Path::new("notes.txt")), None);
        assert_eq!(
            classify_media_type("image", "Screenshot_1.png", None, None, None, Some(1170), Some(2532), None, false),
            "screenshot"
        );
        assert_eq!(
            classify_media_type(
                "image",
                "IMG_0001.jpg",
                Some("Apple"),
                Some("iPhone 14"),
                Some("iPhone 14 Front Camera"),
                Some(4032),
                Some(3024),
                None,
                false
            ),
            "selfie"
        );
        assert_eq!(
            classify_media_type("video", "clip.mp4", None, None, None, Some(1080), Some(1920), Some(15_000), false),
            "short_vertical"
        );
        assert_eq!(
            classify_media_type("video", "family.mp4", None, None, None, Some(1920), Some(1080), Some(600_000), false),
            "video"
        );
    }

    #[test]
    fn offset_applied() {
        let base = ymd_hms_ms(2024, 1, 1, 12, 0, 0).unwrap();
        assert_eq!(apply_offset_ms(base, "+00:00"), base);
        assert_eq!(apply_offset_ms(base, "-05:00"), base + 5 * 3_600_000);
    }
}
