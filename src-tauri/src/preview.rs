//! Fast pass-1 preview from the catalog: time-gap / place events, by-date
//! groups, and type tags. Does not group bursts, Live Photos, or near
//! duplicates (Spin and Angen own those). Never moves files; it only proposes
//! album assignments a later commit step can apply or undo.

use crate::catalog;
use crate::media::{haversine_km, ymd_hms_ms, MediaRecord};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const TWO_HOURS_MS: i64 = 2 * 60 * 60 * 1000;
const PLACE_SPLIT_KM: f64 = 50.0;

#[derive(Debug, Serialize)]
pub struct Preview {
    pub scan_id: String,
    /// Default scheme for Trice / Angen's proposal.schema.json (`groups` below).
    pub scheme: String,
    pub stage: String,
    pub completeness: f64,
    pub schemes: Vec<String>,
    /// Event groups; also exposed as `groups` so the UI can read one shape.
    pub groups: Vec<Group>,
    pub events: Vec<Group>,
    pub dates: Vec<Group>,
    pub types: Vec<Group>,
    /// Angen fills people and near-duplicates; left empty at pass 1.
    pub people: Vec<serde_json::Value>,
    pub duplicates: Vec<serde_json::Value>,
    pub unsorted: Unsorted,
    pub proposals: Vec<Proposal>,
    pub stats: PreviewStats,
    /// Columns teammates fill in the catalog. Present so the UI and commit
    /// step can pass them through without the indexer inventing values.
    pub hooks: HooksInfo,
}

#[derive(Debug, Serialize)]
pub struct Unsorted {
    pub item_ids: Vec<String>,
    pub reason_counts: std::collections::BTreeMap<String, u64>,
}

#[derive(Debug, Serialize)]
pub struct HooksInfo {
    pub group_id: &'static str,
    pub pair_id: &'static str,
    pub phash: &'static str,
    pub faces_table: &'static str,
    pub people_table: &'static str,
}

#[derive(Debug, Serialize, Clone)]
pub struct Group {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub confidence: f64,
    pub reason: Vec<String>,
    pub cover_item_id: Option<String>,
    pub item_ids: Vec<String>,
    pub count: usize,
    pub children: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Proposal {
    pub item_id: String,
    pub path: String,
    pub action: &'static str,
    pub album: String,
    pub scheme: &'static str,
}

#[derive(Debug, Serialize)]
pub struct PreviewStats {
    pub items: usize,
    pub photos: usize,
    pub videos: usize,
    pub screenshots: usize,
    pub selfies: usize,
    pub shorts: usize,
    pub with_exif_date: usize,
    pub with_gps: usize,
}

pub fn build_preview(folder: &str, db: &Path) -> Result<Preview, String> {
    let root = fs::canonicalize(folder)
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    let records = catalog::load_records(db, &root)?;
    Ok(preview_from_records(&root, &records))
}

pub fn write_preview_json(folder: &str, db: &Path, out: &Path) -> Result<Preview, String> {
    let preview = build_preview(folder, db)?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&preview).map_err(|e| e.to_string())?;
    fs::write(out, json).map_err(|e| e.to_string())?;
    Ok(preview)
}

pub fn preview_from_records(root: &str, records: &[MediaRecord]) -> Preview {
    let events = group_events(records);
    let dates = group_dates(records);
    let types = group_types(records);
    let proposals = proposals_for(&events, records);
    let scan_id = format!("s_{}", crate::media::item_id_for(root));
    Preview {
        scan_id,
        scheme: "event".into(),
        stage: "S1_metadata".into(),
        completeness: if records.is_empty() { 0.0 } else { 1.0 },
        schemes: vec!["event".into(), "date".into(), "type".into()],
        groups: events.clone(),
        events,
        dates,
        types,
        people: Vec::new(),
        duplicates: Vec::new(),
        unsorted: Unsorted {
            item_ids: Vec::new(),
            reason_counts: Default::default(),
        },
        proposals,
        stats: stats(records),
        hooks: HooksInfo {
            group_id: "catalog.indexed_media.group_id — Angen fills for near-duplicate / people groups",
            pair_id: "catalog.indexed_media.pair_id — Spin fills for Live Photo, Motion Photo, and burst pairs",
            phash: "catalog.indexed_media.phash — 64-bit dHash, already stored; do not regroup here",
            faces_table: "catalog.faces — Angen writes YuNet/SFace rows keyed by item_path",
            people_table: "catalog.people — Angen writes person clusters and review counts",
        },
    }
}

fn capture_ms(r: &MediaRecord) -> i64 {
    r.captured_ms.unwrap_or(r.modified_ms)
}

fn group_events(records: &[MediaRecord]) -> Vec<Group> {
    if records.is_empty() {
        return Vec::new();
    }
    let mut ordered: Vec<&MediaRecord> = records.iter().collect();
    ordered.sort_by_key(|r| (capture_ms(r), r.path.as_str()));

    let mut gaps: Vec<i64> = Vec::new();
    for pair in ordered.windows(2) {
        let g = (capture_ms(pair[1]) - capture_ms(pair[0])).max(0);
        gaps.push(g);
    }
    let mut sorted_gaps = gaps.clone();
    sorted_gaps.sort_unstable();
    let median = if sorted_gaps.is_empty() {
        TWO_HOURS_MS
    } else {
        sorted_gaps[sorted_gaps.len() / 2]
    };
    let gap_limit = TWO_HOURS_MS.max(median.saturating_mul(3));

    let mut runs: Vec<Vec<&MediaRecord>> = Vec::new();
    let mut cur: Vec<&MediaRecord> = vec![ordered[0]];
    for w in ordered.windows(2) {
        let gap = (capture_ms(w[1]) - capture_ms(w[0])).max(0);
        let place_jump = match (w[0].lat, w[0].lon, w[1].lat, w[1].lon) {
            (Some(a), Some(b), Some(c), Some(d)) => haversine_km(a, b, c, d) > PLACE_SPLIT_KM,
            _ => false,
        };
        if gap > gap_limit || place_jump {
            runs.push(std::mem::take(&mut cur));
        }
        cur.push(w[1]);
    }
    if !cur.is_empty() {
        runs.push(cur);
    }

    // Merge runs shorter than 3 items into neighbors (ENGINE-PLAN event rule).
    let mut merged: Vec<Vec<&MediaRecord>> = Vec::new();
    for run in runs {
        if run.len() < 3 {
            if let Some(prev) = merged.last_mut() {
                prev.extend(run);
                continue;
            }
        }
        merged.push(run);
    }
    if merged.len() >= 2 {
        if let Some(last) = merged.last() {
            if last.len() < 3 {
                let tail = merged.pop().unwrap();
                merged.last_mut().unwrap().extend(tail);
            }
        }
    }

    merged
        .into_iter()
        .enumerate()
        .map(|(i, run)| event_group(i, &run))
        .collect()
}

fn event_group(index: usize, run: &[&MediaRecord]) -> Group {
    let start = run.iter().map(|r| capture_ms(r)).min().unwrap_or(0);
    let end = run.iter().map(|r| capture_ms(r)).max().unwrap_or(start);
    let place = place_title(run);
    let title = format!("{} · {}", place, date_span(start, end));
    let mut reasons = vec!["time_gap".to_string()];
    if run.iter().any(|r| r.lat.is_some()) {
        reasons.push("gps_cluster".into());
    }
    let cover = run
        .iter()
        .max_by_key(|r| r.size)
        .map(|r| r.item_id.clone());
    Group {
        id: format!("g_evt_{index:04}"),
        kind: "event".into(),
        title,
        confidence: 0.85,
        reason: reasons,
        cover_item_id: cover,
        item_ids: run.iter().map(|r| r.item_id.clone()).collect(),
        count: run.len(),
        children: Vec::new(),
    }
}

fn place_title(run: &[&MediaRecord]) -> String {
    let mut cells: BTreeMap<String, usize> = BTreeMap::new();
    for r in run {
        if let (Some(lat), Some(lon)) = (r.lat, r.lon) {
            let key = format!("{:.2}°, {:.2}°", lat, lon);
            *cells.entry(key).or_insert(0) += 1;
        }
    }
    cells
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(k, _)| k)
        .unwrap_or_else(|| "Unknown place".into())
}

fn date_span(start_ms: i64, end_ms: i64) -> String {
    let (sy, sm, sd) = civil(start_ms);
    let (ey, em, ed) = civil(end_ms);
    const MONTHS: &[&str] = &[
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let smon = MONTHS.get((sm.saturating_sub(1)) as usize).copied().unwrap_or("?");
    let emon = MONTHS.get((em.saturating_sub(1)) as usize).copied().unwrap_or("?");
    if sy == ey && sm == em && sd == ed {
        format!("{smon} {sd}, {sy}")
    } else if sy == ey && sm == em {
        format!("{smon} {sd}–{ed}, {sy}")
    } else if sy == ey {
        format!("{smon} {sd} – {emon} {ed}, {sy}")
    } else {
        format!("{smon} {sd}, {sy} – {emon} {ed}, {ey}")
    }
}

fn civil(ms: i64) -> (i32, u32, u32) {
    // Search a modest year range; grouping titles only.
    let days = ms.div_euclid(86_400_000);
    let mut remaining = days;
    let mut y = 1970i32;
    loop {
        let len = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 366 } else { 365 };
        if remaining < len {
            break;
        }
        remaining -= len;
        y += 1;
        if y > 2100 {
            break;
        }
    }
    let mut mo = 1u32;
    while mo <= 12 {
        let Some(anchor) = ymd_hms_ms(y, mo, 1, 0, 0, 0) else { break; };
        let next = if mo == 12 {
            ymd_hms_ms(y + 1, 1, 1, 0, 0, 0)
        } else {
            ymd_hms_ms(y, mo + 1, 1, 0, 0, 0)
        };
        if let Some(n) = next {
            if ms < n && ms >= anchor {
                let d = ((ms - anchor) / 86_400_000) as u32 + 1;
                return (y, mo, d);
            }
        }
        mo += 1;
    }
    (y, 1, 1)
}

fn group_dates(records: &[MediaRecord]) -> Vec<Group> {
    let mut years: BTreeMap<i32, BTreeMap<u32, Vec<&MediaRecord>>> = BTreeMap::new();
    for r in records {
        let (y, m, _) = civil(capture_ms(r));
        years.entry(y).or_default().entry(m).or_default().push(r);
    }
    const MONTHS: &[&str] = &[
        "January", "February", "March", "April", "May", "June", "July", "August",
        "September", "October", "November", "December",
    ];
    let mut groups = Vec::new();
    for (y, months) in years {
        let year_id = format!("g_date_{y}");
        let mut child_ids = Vec::new();
        let mut year_items = Vec::new();
        for (m, items) in months {
            let id = format!("g_date_{y}_{m:02}");
            child_ids.push(id.clone());
            let title = format!(
                "{} {y}",
                MONTHS.get((m.saturating_sub(1)) as usize).copied().unwrap_or("Month")
            );
            year_items.extend(items.iter().copied());
            groups.push(Group {
                id,
                kind: "date_bucket".into(),
                title,
                confidence: 0.95,
                reason: vec!["capture_date".into()],
                cover_item_id: items.iter().max_by_key(|r| r.size).map(|r| r.item_id.clone()),
                item_ids: items.iter().map(|r| r.item_id.clone()).collect(),
                count: items.len(),
                children: Vec::new(),
            });
        }
        groups.push(Group {
            id: year_id,
            kind: "date_bucket".into(),
            title: y.to_string(),
            confidence: 0.95,
            reason: vec!["capture_date".into()],
            cover_item_id: year_items.iter().max_by_key(|r| r.size).map(|r| r.item_id.clone()),
            item_ids: year_items.iter().map(|r| r.item_id.clone()).collect(),
            count: year_items.len(),
            children: child_ids,
        });
    }
    groups
}

fn group_types(records: &[MediaRecord]) -> Vec<Group> {
    let mut map: BTreeMap<String, Vec<&MediaRecord>> = BTreeMap::new();
    for r in records {
        map.entry(r.media_type.clone()).or_default().push(r);
    }
    let title = |t: &str| -> String {
        match t {
            "photo" => "Photos".into(),
            "screenshot" => "Screenshots".into(),
            "selfie" => "Selfies".into(),
            "video" => "Videos".into(),
            "short_vertical" => "Shorts".into(),
            "screen_recording" => "Screen recordings".into(),
            other => other.to_string(),
        }
    };
    map.into_iter()
        .map(|(t, items)| Group {
            id: format!("g_type_{t}"),
            kind: t.clone(),
            title: title(&t),
            confidence: 0.9,
            reason: vec!["type_tag".into()],
            cover_item_id: items.iter().max_by_key(|r| r.size).map(|r| r.item_id.clone()),
            item_ids: items.iter().map(|r| r.item_id.clone()).collect(),
            count: items.len(),
            children: Vec::new(),
        })
        .collect()
}

fn proposals_for(events: &[Group], records: &[MediaRecord]) -> Vec<Proposal> {
    let mut by_id: BTreeMap<&str, &MediaRecord> = BTreeMap::new();
    for r in records {
        by_id.insert(r.item_id.as_str(), r);
    }
    let mut out = Vec::new();
    for g in events {
        for id in &g.item_ids {
            if let Some(r) = by_id.get(id.as_str()) {
                out.push(Proposal {
                    item_id: r.item_id.clone(),
                    path: r.path.clone(),
                    action: "album_assign",
                    album: format!("Events/{}", g.title),
                    scheme: "event",
                });
            }
        }
    }
    out
}

fn stats(records: &[MediaRecord]) -> PreviewStats {
    let mut s = PreviewStats {
        items: records.len(),
        photos: 0,
        videos: 0,
        screenshots: 0,
        selfies: 0,
        shorts: 0,
        with_exif_date: 0,
        with_gps: 0,
    };
    for r in records {
        match r.media_type.as_str() {
            "screenshot" => s.screenshots += 1,
            "selfie" => s.selfies += 1,
            "short_vertical" => s.shorts += 1,
            "video" | "screen_recording" => s.videos += 1,
            _ => s.photos += 1,
        }
        if r.kind == "video" && r.media_type == "short_vertical" {
            // already counted as shorts, not videos
        }
        if r.kind == "video" && r.media_type != "short_vertical" {
            // videos counted in match
        }
        if r.date_source == "exif" {
            s.with_exif_date += 1;
        }
        if r.lat.is_some() && r.lon.is_some() {
            s.with_gps += 1;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::ymd_hms_ms;

    fn rec(id: &str, ms: i64, lat: Option<f64>, lon: Option<f64>, typ: &str) -> MediaRecord {
        MediaRecord {
            item_id: id.into(),
            path: format!("/tmp/{id}.jpg"),
            root: "/tmp".into(),
            name: format!("{id}.jpg"),
            size: 100,
            modified_ms: ms,
            kind: "image".into(),
            indexed_ms: ms,
            captured_ms: Some(ms),
            date_source: "exif".into(),
            date_confidence: 0.9,
            lat,
            lon,
            camera_make: None,
            camera_model: None,
            lens: None,
            width: Some(100),
            height: Some(80),
            duration_ms: None,
            media_type: typ.into(),
            phash: None,
            thumb_path: None,
            front_camera: false,
            group_id: None,
            pair_id: None,
        }
    }

    #[test]
    fn splits_on_time_gap_and_keeps_same_day_together() {
        let a = ymd_hms_ms(2024, 6, 3, 10, 0, 0).unwrap();
        let b = ymd_hms_ms(2024, 6, 3, 11, 0, 0).unwrap();
        let c = ymd_hms_ms(2024, 6, 3, 11, 20, 0).unwrap();
        let d = ymd_hms_ms(2024, 6, 10, 9, 0, 0).unwrap();
        let e = ymd_hms_ms(2024, 6, 10, 9, 10, 0).unwrap();
        let f = ymd_hms_ms(2024, 6, 10, 9, 20, 0).unwrap();
        let recs = vec![
            rec("a", a, Some(38.7), Some(-9.14), "photo"),
            rec("b", b, Some(38.7), Some(-9.14), "photo"),
            rec("c", c, Some(38.7), Some(-9.14), "photo"),
            rec("d", d, Some(40.4), Some(-3.7), "photo"),
            rec("e", e, Some(40.4), Some(-3.7), "photo"),
            rec("f", f, Some(40.4), Some(-3.7), "photo"),
        ];
        let p = preview_from_records("/tmp", &recs);
        assert!(p.events.len() >= 2, "expected at least two events, got {}", p.events.len());
        assert!(p.dates.iter().any(|g| g.title.contains("2024")));
        assert!(p.types.iter().any(|g| g.kind == "photo"));
        assert_eq!(p.proposals.len(), 6);
        assert!(p.proposals.iter().all(|x| x.action == "album_assign"));
    }
}
