//! Cheap metadata reads: EXIF (kamadak-exif), embedded JPEG thumbnails,
//! container dimensions, and MP4/MOV duration. Original files are never written.

use crate::media::{
    apply_offset_ms, date_from_filename, looks_front_camera, ymd_hms_ms,
};
use exif::{Exif, In, Reader, Tag, Value};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const META_PREFIX: usize = 128 * 1024;

#[derive(Debug, Clone, Default)]
pub struct FileMeta {
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
    pub front_camera: bool,
    pub exif_thumb: Option<Vec<u8>>,
}

pub fn read_file_meta(path: &Path, kind: &str, name: &str, modified_ms: i64) -> FileMeta {
    let mut meta = FileMeta {
        date_source: "mtime".into(),
        date_confidence: 0.25,
        captured_ms: if modified_ms > 0 { Some(modified_ms) } else { None },
        ..FileMeta::default()
    };

    if let Some((ms, conf)) = date_from_filename(name) {
        meta.captured_ms = Some(ms);
        meta.date_source = "filename".into();
        meta.date_confidence = conf;
    }

    let prefix = read_prefix(path, META_PREFIX);
    if let Some(ref bytes) = prefix {
        if let Some((w, h)) = image_dimensions(bytes, path) {
            meta.width = Some(w);
            meta.height = Some(h);
        }
        if let Some(thumb) = extract_exif_jpeg_thumbnail(bytes) {
            meta.exif_thumb = Some(thumb);
        }
        if let Ok(exif) = parse_exif(bytes) {
            apply_exif(&exif, &mut meta);
        } else if let Some(fields) = parse_exif_lenient(bytes) {
            // kamadak-exif rejects some valid writers (e.g. piexif thumbnail IFDs
            // with "Unexpected next IFD"). Fall back to a small IFD walker.
            apply_lenient(fields, &mut meta);
        }
    }

    if kind == "video" {
        if let Some((dur, w, h)) = parse_video_meta(path) {
            meta.duration_ms = dur;
            if meta.width.is_none() {
                meta.width = w;
            }
            if meta.height.is_none() {
                meta.height = h;
            }
        }
    }

    meta.front_camera = looks_front_camera(&[
        meta.camera_make.as_deref().unwrap_or(""),
        meta.camera_model.as_deref().unwrap_or(""),
        meta.lens.as_deref().unwrap_or(""),
    ]);
    meta
}

fn read_prefix(path: &Path, n: usize) -> Option<Vec<u8>> {
    let mut f = File::open(path).ok()?;
    let mut buf = vec![0u8; n];
    let got = f.read(&mut buf).ok()?;
    buf.truncate(got);
    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
}

fn parse_exif(bytes: &[u8]) -> Result<Exif, exif::Error> {
    Reader::new().read_from_container(&mut std::io::Cursor::new(bytes))
}

fn field_text(exif: &Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    let s = field.display_value().with_unit(exif).to_string();
    let s = s.trim().trim_matches('"').trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn field_ascii(exif: &Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    match &field.value {
        Value::Ascii(v) if !v.is_empty() => {
            let s = String::from_utf8_lossy(&v[0]).trim().trim_end_matches('\0').to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        _ => field_text(exif, tag),
    }
}

fn apply_exif(exif: &Exif, meta: &mut FileMeta) {
    if let Some(s) = field_ascii(exif, Tag::Make).or_else(|| field_text(exif, Tag::Make)) {
        meta.camera_make = Some(s);
    }
    if let Some(s) = field_ascii(exif, Tag::Model).or_else(|| field_text(exif, Tag::Model)) {
        meta.camera_model = Some(s);
    }
    if let Some(s) = field_ascii(exif, Tag::LensModel)
        .or_else(|| field_text(exif, Tag::LensModel))
        .or_else(|| field_ascii(exif, Tag::LensMake))
    {
        meta.lens = Some(s);
    }

    if let Some((w, h)) = exif_dimensions(exif) {
        if meta.width.is_none() {
            meta.width = Some(w);
        }
        if meta.height.is_none() {
            meta.height = Some(h);
        }
    }

    if let Some((lat, lon)) = gps_pair(exif) {
        meta.lat = Some(lat);
        meta.lon = Some(lon);
    }

    if let Some(ms) = exif_captured_ms(exif) {
        meta.captured_ms = Some(ms);
        meta.date_source = "exif".into();
        meta.date_confidence = 0.98;
    }
}

fn exif_dimensions(exif: &Exif) -> Option<(i64, i64)> {
    let w = uint_field(exif, Tag::PixelXDimension)
        .or_else(|| uint_field(exif, Tag::ImageWidth))?;
    let h = uint_field(exif, Tag::PixelYDimension)
        .or_else(|| uint_field(exif, Tag::ImageLength))?;
    if w > 0 && h > 0 {
        Some((w as i64, h as i64))
    } else {
        None
    }
}

fn uint_field(exif: &Exif, tag: Tag) -> Option<u32> {
    exif.get_field(tag, In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
}

fn exif_captured_ms(exif: &Exif) -> Option<i64> {
    let raw = field_ascii(exif, Tag::DateTimeOriginal)
        .or_else(|| field_ascii(exif, Tag::DateTimeDigitized))
        .or_else(|| field_ascii(exif, Tag::DateTime))?;
    let ms = parse_exif_datetime(&raw)?;
    let offset = field_ascii(exif, Tag::OffsetTimeOriginal)
        .or_else(|| field_ascii(exif, Tag::OffsetTimeDigitized))
        .or_else(|| field_ascii(exif, Tag::OffsetTime));
    Some(match offset {
        Some(off) => apply_offset_ms(ms, &off),
        None => ms,
    })
}

pub fn parse_exif_datetime(raw: &str) -> Option<i64> {
    // "2024:06:03 14:22:01" and minor variants
    let cleaned: String = raw
        .chars()
        .map(|c| if c == '-' { ':' } else { c })
        .collect();
    let cleaned = cleaned.replace('T', " ");
    let mut parts = cleaned.split_whitespace();
    let date = parts.next()?;
    let time = parts.next().unwrap_or("00:00:00");
    let mut d = date.split(':');
    let y: i32 = d.next()?.parse().ok()?;
    let mo: u32 = d.next()?.parse().ok()?;
    let da: u32 = d.next()?.parse().ok()?;
    let mut t = time.split(':');
    let h: u32 = t.next()?.parse().ok()?;
    let mi: u32 = t.next()?.parse().ok()?;
    let se_raw = t.next().unwrap_or("0");
    let se: u32 = se_raw
        .split(['.', '+', '-'])
        .next()?
        .parse()
        .ok()?;
    ymd_hms_ms(y, mo, da, h, mi, se)
}

fn gps_pair(exif: &Exif) -> Option<(f64, f64)> {
    let lat = gps_coord(exif, Tag::GPSLatitude, Tag::GPSLatitudeRef)?;
    let lon = gps_coord(exif, Tag::GPSLongitude, Tag::GPSLongitudeRef)?;
    if lat.abs() <= 90.0 && lon.abs() <= 180.0 {
        Some((lat, lon))
    } else {
        None
    }
}

fn gps_coord(exif: &Exif, tag: Tag, ref_tag: Tag) -> Option<f64> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    let rats = match &field.value {
        Value::Rational(v) if v.len() >= 3 => v,
        _ => return None,
    };
    let deg = rat_f64(rats[0])?;
    let min = rat_f64(rats[1])?;
    let sec = rat_f64(rats[2])?;
    let mut dec = deg + min / 60.0 + sec / 3600.0;
    if let Some(r) = exif.get_field(ref_tag, In::PRIMARY) {
        let s = match &r.value {
            Value::Ascii(v) if !v.is_empty() => String::from_utf8_lossy(&v[0]).to_ascii_uppercase(),
            _ => r.display_value().to_string().to_ascii_uppercase(),
        };
        if s.starts_with('S') || s.starts_with('W') {
            dec = -dec;
        }
    }
    Some(dec)
}

fn rat_f64(r: exif::Rational) -> Option<f64> {
    if r.denom == 0 {
        None
    } else {
        Some(r.num as f64 / r.denom as f64)
    }
}

/// Walk a JPEG prefix for SOF dimensions. Also handles PNG IHDR.
pub fn image_dimensions(bytes: &[u8], path: &Path) -> Option<(i64, i64)> {
    if bytes.len() >= 24 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?) as i64;
        let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?) as i64;
        if w > 0 && h > 0 {
            return Some((w, h));
        }
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        if let Some((w, h)) = jpeg_sof_size(bytes) {
            return Some((w, h));
        }
    }
    // WebP VP8X / VP8
    if bytes.len() >= 30 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        if let Some(wh) = webp_size(bytes) {
            return Some(wh);
        }
    }
    // fallback by extension only if we already have nothing
    let _ = path;
    None
}

fn jpeg_sof_size(bytes: &[u8]) -> Option<(i64, i64)> {
    let mut i = 2usize;
    while i + 4 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        if marker == 0xD9 {
            break;
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if len < 2 || i + 2 + len > bytes.len() {
            break;
        }
        // SOF0..SOF3, SOF5..SOF7, SOF9..SOF11, SOF13..SOF15
        if (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC {
            if len >= 7 && i + 9 <= bytes.len() {
                let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as i64;
                let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as i64;
                if w > 0 && h > 0 {
                    return Some((w, h));
                }
            }
        }
        i += 2 + len;
    }
    None
}

fn webp_size(bytes: &[u8]) -> Option<(i64, i64)> {
    if bytes.len() >= 30 && &bytes[12..16] == b"VP8X" {
        let w = 1 + (bytes[24] as i64 | (bytes[25] as i64) << 8 | (bytes[26] as i64) << 16);
        let h = 1 + (bytes[27] as i64 | (bytes[28] as i64) << 8 | (bytes[29] as i64) << 16);
        return Some((w, h));
    }
    if bytes.len() >= 30 && &bytes[12..16] == b"VP8 " {
        // lossy: 26-27 width, 28-29 height, 14-bit
        let w = (u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3FFF) as i64;
        let h = (u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3FFF) as i64;
        if w > 0 && h > 0 {
            return Some((w, h));
        }
    }
    None
}

/// Extract the JPEG thumbnail stored in IFD1 of an APP1 Exif segment.
pub fn extract_exif_jpeg_thumbnail(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 12 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut i = 2usize;
    while i + 4 < bytes.len() {
        if bytes[i] != 0xFF {
            break;
        }
        let marker = bytes[i + 1];
        if marker == 0xDA || marker == 0xD9 {
            break;
        }
        if marker == 0xD8 {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if len < 2 || i + 2 + len > bytes.len() {
            break;
        }
        if marker == 0xE1 {
            let payload = &bytes[i + 4..i + 2 + len];
            if payload.starts_with(b"Exif\0\0") {
                if let Some(thumb) = tiff_ifd1_jpeg(&payload[6..]) {
                    return Some(thumb);
                }
            }
        }
        i += 2 + len;
    }
    None
}

fn tiff_ifd1_jpeg(tiff: &[u8]) -> Option<Vec<u8>> {
    if tiff.len() < 8 {
        return None;
    }
    let le = match &tiff[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16v = |o: usize| -> Option<u16> {
        let b = tiff.get(o..o + 2)?;
        Some(if le {
            u16::from_le_bytes(b.try_into().ok()?)
        } else {
            u16::from_be_bytes(b.try_into().ok()?)
        })
    };
    let u32v = |o: usize| -> Option<u32> {
        let b = tiff.get(o..o + 4)?;
        Some(if le {
            u32::from_le_bytes(b.try_into().ok()?)
        } else {
            u32::from_be_bytes(b.try_into().ok()?)
        })
    };
    if u16v(2)? != 42 {
        return None;
    }
    let ifd0 = u32v(4)? as usize;
    let (ifd1, _) = read_ifd_next(tiff, ifd0, le)?;
    if ifd1 == 0 {
        return None;
    }
    let (_, entries) = read_ifd_next(tiff, ifd1 as usize, le)?;
    let mut off: Option<u32> = None;
    let mut len: Option<u32> = None;
    for (tag, typ, count, value) in entries {
        // JPEGInterchangeFormat 0x0201, Length 0x0202
        if tag == 0x0201 {
            off = Some(if typ == 3 && count == 1 && le {
                value & 0xFFFF
            } else if typ == 3 && count == 1 {
                value >> 16
            } else {
                value
            });
        }
        if tag == 0x0202 {
            len = Some(if typ == 3 && count == 1 && le {
                value & 0xFFFF
            } else if typ == 3 && count == 1 {
                value >> 16
            } else {
                value
            });
        }
    }
    let off = off? as usize;
    let len = len? as usize;
    if len == 0 || off + len > tiff.len() {
        return None;
    }
    let thumb = tiff[off..off + len].to_vec();
    if thumb.starts_with(&[0xFF, 0xD8]) {
        Some(thumb)
    } else {
        None
    }
}

fn read_ifd_next(tiff: &[u8], offset: usize, le: bool) -> Option<(u32, Vec<(u16, u16, u32, u32)>)> {
    if offset + 2 > tiff.len() {
        return None;
    }
    let count = if le {
        u16::from_le_bytes(tiff[offset..offset + 2].try_into().ok()?)
    } else {
        u16::from_be_bytes(tiff[offset..offset + 2].try_into().ok()?)
    } as usize;
    let mut entries = Vec::with_capacity(count);
    let mut p = offset + 2;
    for _ in 0..count {
        if p + 12 > tiff.len() {
            break;
        }
        let tag = if le {
            u16::from_le_bytes(tiff[p..p + 2].try_into().ok()?)
        } else {
            u16::from_be_bytes(tiff[p..p + 2].try_into().ok()?)
        };
        let typ = if le {
            u16::from_le_bytes(tiff[p + 2..p + 4].try_into().ok()?)
        } else {
            u16::from_be_bytes(tiff[p + 2..p + 4].try_into().ok()?)
        };
        let cnt = if le {
            u32::from_le_bytes(tiff[p + 4..p + 8].try_into().ok()?)
        } else {
            u32::from_be_bytes(tiff[p + 4..p + 8].try_into().ok()?)
        };
        let val = if le {
            u32::from_le_bytes(tiff[p + 8..p + 12].try_into().ok()?)
        } else {
            u32::from_be_bytes(tiff[p + 8..p + 12].try_into().ok()?)
        };
        entries.push((tag, typ, cnt, val));
        p += 12;
    }
    if p + 4 > tiff.len() {
        return Some((0, entries));
    }
    let next = if le {
        u32::from_le_bytes(tiff[p..p + 4].try_into().ok()?)
    } else {
        u32::from_be_bytes(tiff[p..p + 4].try_into().ok()?)
    };
    Some((next, entries))
}

/// Duration (ms) and optional display size from MP4/MOV boxes.
/// Looks at the first and last megabyte so a trailing `moov` is still found.
pub fn parse_video_meta(path: &Path) -> Option<(Option<i64>, Option<i64>, Option<i64>)> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    if len < 16 {
        return None;
    }
    let head_n = len.min(1024 * 1024) as usize;
    let mut head = vec![0u8; head_n];
    f.read_exact(&mut head).ok()?;
    if let Some(found) = scan_mp4_moov(&head, true) {
        return Some(found);
    }
    if len > 1024 * 1024 {
        let tail_n = (1024 * 1024).min(len as usize);
        let mut tail = vec![0u8; tail_n];
        f.seek(SeekFrom::End(-(tail_n as i64))).ok()?;
        f.read_exact(&mut tail).ok()?;
        if let Some(found) = scan_mp4_moov(&tail, true) {
            return Some(found);
        }
    }
    // Cheap ffprobe fallback for non-ISO containers (webm, mkv, avi).
    if let Some(dur) = ffprobe_duration_ms(path) {
        return Some((Some(dur), None, None));
    }
    None
}

fn scan_mp4_moov(buf: &[u8], top: bool) -> Option<(Option<i64>, Option<i64>, Option<i64>)> {
    let mut i = 0usize;
    let mut duration = None;
    let mut width = None;
    let mut height = None;
    while i + 8 <= buf.len() {
        let mut size = u32::from_be_bytes(buf[i..i + 4].try_into().ok()?) as u64;
        let typ = &buf[i + 4..i + 8];
        let mut hdr = 8usize;
        if size == 1 {
            if i + 16 > buf.len() {
                break;
            }
            size = u64::from_be_bytes(buf[i + 8..i + 16].try_into().ok()?);
            hdr = 16;
        } else if size == 0 {
            size = (buf.len() - i) as u64;
        }
        if size < hdr as u64 {
            break;
        }
        let end = (i as u64 + size).min(buf.len() as u64) as usize;
        if typ == b"moov" || typ == b"trak" || typ == b"mdia" {
            if let Some((d, w, h)) = scan_mp4_moov(&buf[i + hdr..end], false) {
                duration = duration.or(d);
                width = width.or(w);
                height = height.or(h);
            }
        } else if typ == b"mvhd" {
            duration = duration.or(parse_mvhd(&buf[i + hdr..end]));
        } else if typ == b"tkhd" {
            if let Some((w, h)) = parse_tkhd(&buf[i + hdr..end]) {
                if width.is_none() && w > 0 && h > 0 {
                    width = Some(w);
                    height = Some(h);
                }
            }
        } else if top && typ != b"ftyp" && typ != b"free" && typ != b"mdat" && typ != b"wide" && typ != b"skip" && typ != b"uuid" && typ != b"pnot" && typ != b"junk"
        {
            // keep walking top-level boxes we don't recognize
        }
        if size == 0 {
            break;
        }
        i = i.saturating_add(size as usize);
        if i <= hdr {
            break;
        }
    }
    if duration.is_some() || width.is_some() {
        Some((duration, width, height))
    } else {
        None
    }
}

fn parse_mvhd(body: &[u8]) -> Option<i64> {
    if body.is_empty() {
        return None;
    }
    let version = body[0];
    if version == 1 {
        if body.len() < 32 {
            return None;
        }
        let timescale = u32::from_be_bytes(body[20..24].try_into().ok()?) as u64;
        let duration = u64::from_be_bytes(body[24..32].try_into().ok()?);
        if timescale == 0 {
            return None;
        }
        Some((duration.saturating_mul(1000) / timescale) as i64)
    } else {
        if body.len() < 20 {
            return None;
        }
        let timescale = u32::from_be_bytes(body[12..16].try_into().ok()?) as u64;
        let duration = u32::from_be_bytes(body[16..20].try_into().ok()?) as u64;
        if timescale == 0 {
            return None;
        }
        Some((duration.saturating_mul(1000) / timescale) as i64)
    }
}

fn parse_tkhd(body: &[u8]) -> Option<(i64, i64)> {
    if body.is_empty() {
        return None;
    }
    let version = body[0];
    // width/height are the last two 32-bit 16.16 values
    let min = if version == 1 { 96 } else { 84 };
    if body.len() < min {
        return None;
    }
    let base = if version == 1 { 88 } else { 76 };
    if body.len() < base + 8 {
        return None;
    }
    let w = (u32::from_be_bytes(body[base..base + 4].try_into().ok()?) >> 16) as i64;
    let h = (u32::from_be_bytes(body[base + 4..base + 8].try_into().ok()?) >> 16) as i64;
    Some((w, h))
}

fn ffprobe_duration_ms(path: &Path) -> Option<i64> {
    let out = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let secs: f64 = s.parse().ok()?;
    if secs.is_finite() && secs >= 0.0 {
        Some((secs * 1000.0).round() as i64)
    } else {
        None
    }
}


#[derive(Default, Debug)]
struct LenientExif {
    make: Option<String>,
    model: Option<String>,
    lens: Option<String>,
    datetime_original: Option<String>,
    datetime: Option<String>,
    offset: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    lat: Option<f64>,
    lon: Option<f64>,
}

/// Minimal TIFF/APP1 walker used when kamadak-exif rejects a file.
fn parse_exif_lenient(bytes: &[u8]) -> Option<LenientExif> {
    let tiff = app1_exif_tiff(bytes)?;
    parse_tiff_fields(&tiff)
}

fn app1_exif_tiff(bytes: &[u8]) -> Option<&[u8]> {
    if bytes.len() < 12 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut i = 2usize;
    while i + 4 < bytes.len() {
        if bytes[i] != 0xFF {
            break;
        }
        let marker = bytes[i + 1];
        if marker == 0xDA || marker == 0xD9 {
            break;
        }
        if marker == 0xD8 {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if len < 2 || i + 2 + len > bytes.len() {
            break;
        }
        if marker == 0xE1 {
            let payload = &bytes[i + 4..i + 2 + len];
            if payload.starts_with(b"Exif\0\0") && payload.len() > 6 {
                return Some(&payload[6..]);
            }
        }
        i += 2 + len;
    }
    None
}

fn parse_tiff_fields(tiff: &[u8]) -> Option<LenientExif> {
    if tiff.len() < 8 {
        return None;
    }
    let le = match &tiff[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    if u16_at(tiff, 2, le)? != 42 {
        return None;
    }
    let ifd0 = u32_at(tiff, 4, le)? as usize;
    let entries = ifd_entries(tiff, ifd0, le)?;
    let mut out = LenientExif::default();
    let mut exif_off = None;
    let mut gps_off = None;
    for e in &entries {
        match e.tag {
            0x010F => out.make = ifd_ascii(tiff, e, le),
            0x0110 => out.model = ifd_ascii(tiff, e, le),
            0x0132 => out.datetime = ifd_ascii(tiff, e, le),
            0x8769 => exif_off = ifd_u32(e, le),
            0x8825 => gps_off = ifd_u32(e, le),
            _ => {}
        }
    }
    if let Some(off) = exif_off {
        if let Some(exif_entries) = ifd_entries(tiff, off as usize, le) {
            for e in &exif_entries {
                match e.tag {
                    0x9003 => out.datetime_original = ifd_ascii(tiff, e, le),
                    0x9004 => {
                        if out.datetime_original.is_none() {
                            out.datetime_original = ifd_ascii(tiff, e, le);
                        }
                    }
                    0x9010 | 0x9011 | 0x9012 => {
                        if out.offset.is_none() {
                            out.offset = ifd_ascii(tiff, e, le);
                        }
                    }
                    0xA434 => out.lens = ifd_ascii(tiff, e, le),
                    0xA433 => {
                        if out.lens.is_none() {
                            out.lens = ifd_ascii(tiff, e, le);
                        }
                    }
                    0xA002 => out.width = ifd_u32(e, le).map(|v| v as i64),
                    0xA003 => out.height = ifd_u32(e, le).map(|v| v as i64),
                    _ => {}
                }
            }
        }
    }
    if let Some(off) = gps_off {
        if let Some(gps_entries) = ifd_entries(tiff, off as usize, le) {
            let mut lat_ref = None;
            let mut lon_ref = None;
            let mut lat = None;
            let mut lon = None;
            for e in &gps_entries {
                match e.tag {
                    0x0001 => lat_ref = ifd_ascii(tiff, e, le),
                    0x0003 => lon_ref = ifd_ascii(tiff, e, le),
                    0x0002 => lat = ifd_gps_coord(tiff, e, le),
                    0x0004 => lon = ifd_gps_coord(tiff, e, le),
                    _ => {}
                }
            }
            if let (Some(mut la), Some(mut lo)) = (lat, lon) {
                if matches!(lat_ref.as_deref(), Some(r) if r.starts_with('S') || r.starts_with('s')) {
                    la = -la;
                }
                if matches!(lon_ref.as_deref(), Some(r) if r.starts_with('W') || r.starts_with('w')) {
                    lo = -lo;
                }
                if la.abs() <= 90.0 && lo.abs() <= 180.0 {
                    out.lat = Some(la);
                    out.lon = Some(lo);
                }
            }
        }
    }
    Some(out)
}

struct IfdEntry {
    tag: u16,
    typ: u16,
    count: u32,
    value: u32,
}

fn ifd_entries(tiff: &[u8], offset: usize, le: bool) -> Option<Vec<IfdEntry>> {
    if offset + 2 > tiff.len() {
        return None;
    }
    let count = u16_at(tiff, offset, le)? as usize;
    let mut entries = Vec::with_capacity(count);
    let mut p = offset + 2;
    for _ in 0..count {
        if p + 12 > tiff.len() {
            break;
        }
        entries.push(IfdEntry {
            tag: u16_at(tiff, p, le)?,
            typ: u16_at(tiff, p + 2, le)?,
            count: u32_at(tiff, p + 4, le)?,
            value: u32_at(tiff, p + 8, le)?,
        });
        p += 12;
    }
    Some(entries)
}

fn u16_at(b: &[u8], o: usize, le: bool) -> Option<u16> {
    let s = b.get(o..o + 2)?;
    Some(if le {
        u16::from_le_bytes(s.try_into().ok()?)
    } else {
        u16::from_be_bytes(s.try_into().ok()?)
    })
}

fn u32_at(b: &[u8], o: usize, le: bool) -> Option<u32> {
    let s = b.get(o..o + 4)?;
    Some(if le {
        u32::from_le_bytes(s.try_into().ok()?)
    } else {
        u32::from_be_bytes(s.try_into().ok()?)
    })
}

fn ifd_u32(e: &IfdEntry, le: bool) -> Option<u32> {
    match (e.typ, e.count) {
        (3, 1) => Some(if le { e.value & 0xFFFF } else { e.value >> 16 }),
        (4, 1) | (13, 1) => Some(e.value),
        _ => None,
    }
}

fn ifd_ascii(tiff: &[u8], e: &IfdEntry, le: bool) -> Option<String> {
    if e.typ != 2 || e.count == 0 {
        return None;
    }
    let bytes = if e.count <= 4 {
        let raw = if le {
            e.value.to_le_bytes()
        } else {
            e.value.to_be_bytes()
        };
        raw[..e.count as usize].to_vec()
    } else {
        let off = e.value as usize;
        tiff.get(off..off + e.count as usize)?.to_vec()
    };
    let s = String::from_utf8_lossy(&bytes)
        .trim_end_matches('\0')
        .trim()
        .to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn ifd_gps_coord(tiff: &[u8], e: &IfdEntry, le: bool) -> Option<f64> {
    if e.typ != 5 || e.count < 3 {
        return None;
    }
    let off = e.value as usize;
    if off + 24 > tiff.len() {
        return None;
    }
    let rat = |i: usize| -> Option<f64> {
        let base = off + i * 8;
        let num = u32_at(tiff, base, le)? as f64;
        let den = u32_at(tiff, base + 4, le)? as f64;
        if den == 0.0 {
            None
        } else {
            Some(num / den)
        }
    };
    Some(rat(0)? + rat(1)? / 60.0 + rat(2)? / 3600.0)
}

fn apply_lenient(fields: LenientExif, meta: &mut FileMeta) {
    if let Some(s) = fields.make {
        meta.camera_make = Some(s);
    }
    if let Some(s) = fields.model {
        meta.camera_model = Some(s);
    }
    if let Some(s) = fields.lens {
        meta.lens = Some(s);
    }
    if let (Some(w), Some(h)) = (fields.width, fields.height) {
        if meta.width.is_none() {
            meta.width = Some(w);
        }
        if meta.height.is_none() {
            meta.height = Some(h);
        }
    }
    if let (Some(lat), Some(lon)) = (fields.lat, fields.lon) {
        meta.lat = Some(lat);
        meta.lon = Some(lon);
    }
    if let Some(raw) = fields.datetime_original.or(fields.datetime) {
        if let Some(ms) = parse_exif_datetime(&raw) {
            let ms = match fields.offset.as_deref() {
                Some(off) => apply_offset_ms(ms, off),
                None => ms,
            };
            meta.captured_ms = Some(ms);
            meta.date_source = "exif".into();
            meta.date_confidence = 0.98;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::ymd_hms_ms;
    use std::path::Path;

    #[test]
    fn parses_exif_datetime_formats() {
        assert_eq!(
            parse_exif_datetime("2024:06:03 14:22:01"),
            ymd_hms_ms(2024, 6, 3, 14, 22, 1)
        );
    }

    #[test]
    fn jpeg_size_from_sof() {
        let mut jpeg = vec![0xFF, 0xD8];
        jpeg.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x10, 0x00, 0x20, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xD9]);
        assert_eq!(jpeg_sof_size(&jpeg), Some((32, 16)));
    }

    #[test]
    fn lenient_parser_reads_piexif_style_jpeg() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exif_gps_thumb.jpg");
        let bytes = std::fs::read(&path).expect("fixture");
        let fields = parse_exif_lenient(&bytes).expect("lenient parse");
        assert_eq!(fields.make.as_deref(), Some("Canon"));
        assert_eq!(fields.model.as_deref(), Some("EOS R6"));
        assert!(fields.datetime_original.as_deref().unwrap().starts_with("2024:06:03"));
        let lat = fields.lat.unwrap();
        let lon = fields.lon.unwrap();
        assert!((lat - 38.7223).abs() < 0.001, "lat {lat}");
        assert!((lon + 9.1393).abs() < 0.001, "lon {lon}");
        let mut meta = FileMeta::default();
        if let Ok(exif) = parse_exif(&bytes) {
            apply_exif(&exif, &mut meta);
        } else if let Some(f) = parse_exif_lenient(&bytes) {
            apply_lenient(f, &mut meta);
        }
        assert_eq!(meta.date_source, "exif");
        assert!(meta.captured_ms.is_some());
        assert!(meta.lat.is_some() && meta.lon.is_some());
        assert!(extract_exif_jpeg_thumbnail(&bytes).is_some());
    }
}
