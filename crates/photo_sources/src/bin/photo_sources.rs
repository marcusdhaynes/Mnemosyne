//! Small CLI for testing adapters, unpacking, enumeration, commit and undo.
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use photo_sources::commit::{self, CommitMode, CommitPlan, PlanOp};
use photo_sources::formats::PassThroughResolver;
use photo_sources::sources::{self, cloud};
use photo_sources::{enumerate_path, unpack_export};

fn usage() -> ! {
    eprintln!(
        "photo_sources — Mnemosyne sources and commit
Usage:
  photo_sources discover [ROOT...]
  photo_sources enumerate <PATH> [--ndjson FILE] [--json FILE]
  photo_sources unpack <ZIP> <DEST>
  photo_sources cloud
  photo_sources commit-dry <PLAN.json>
  photo_sources commit-apply <PLAN.json> <JOURNAL.json>
  photo_sources undo <JOURNAL.json>
  photo_sources bench [--count N] [--dir DIR]
"
    );
    std::process::exit(2);
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    let cmd = args.remove(0);
    if let Err(e) = dispatch(&cmd, &args) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            return args.get(i + 1).cloned();
        }
        i += 1;
    }
    None
}

fn dispatch(cmd: &str, args: &[String]) -> photo_sources::Result<()> {
    match cmd {
        "discover" => {
            let roots: Vec<PathBuf> = args.iter().map(PathBuf::from).collect();
            let found = sources::discover_devices(&roots)?;
            println!("{}", serde_json::to_string_pretty(&found)?);
        }
        "enumerate" => {
            let path = args.first().map(PathBuf::from).ok_or_else(|| {
                photo_sources::Error::Message("enumerate requires a path".into())
            })?;
            let started = Instant::now();
            let enumeration = enumerate_path(&path, &PassThroughResolver)?;
            let ms = started.elapsed().as_millis();
            if let Some(out) = flag(args, "--ndjson") {
                let mut f = fs::File::create(&out).map_err(|e| photo_sources::error::io_at(&out, e))?;
                sources::write_ndjson(&enumeration, &mut f)?;
            }
            let summary = serde_json::json!({
                "root": enumeration.root,
                "source_id": enumeration.source_id,
                "source_kind": enumeration.source_kind,
                "export_kind": enumeration.export_kind,
                "device_kind": enumeration.device_kind,
                "label": enumeration.label,
                "media_count": enumeration.media_count,
                "sidecar_count": enumeration.sidecar_count,
                "skipped_other": enumeration.skipped_other,
                "elapsed_ms": ms,
            });
            if let Some(out) = flag(args, "--json") {
                fs::write(&out, serde_json::to_string_pretty(&enumeration)?)
                    .map_err(|e| photo_sources::error::io_at(&out, e))?;
            }
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        "unpack" => {
            let zip = args.first().map(PathBuf::from).ok_or_else(|| {
                photo_sources::Error::Message("unpack requires a zip and a destination".into())
            })?;
            let dest = args.get(1).map(PathBuf::from).ok_or_else(|| {
                photo_sources::Error::Message("unpack requires a destination folder".into())
            })?;
            let report = unpack_export(&zip, &dest)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "cloud" => {
            println!("{}", serde_json::to_string_pretty(&cloud::list_stubs())?);
        }
        "commit-dry" => {
            let plan = load_plan(args.first())?;
            let report = commit::dry_run(&plan)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.ok {
                std::process::exit(1);
            }
        }
        "commit-apply" => {
            let plan = load_plan(args.first())?;
            let journal = args.get(1).map(PathBuf::from).ok_or_else(|| {
                photo_sources::Error::Message("commit-apply requires a journal path".into())
            })?;
            let journal = commit::apply(&plan, &journal)?;
            println!("{}", serde_json::to_string_pretty(&journal)?);
        }
        "undo" => {
            let path = args.first().map(PathBuf::from).ok_or_else(|| {
                photo_sources::Error::Message("undo requires a journal path".into())
            })?;
            let journal = commit::undo(&path)?;
            println!("{}", serde_json::to_string_pretty(&journal)?);
        }
        "bench" => {
            let count: usize = flag(args, "--count")
                .and_then(|s| s.parse().ok())
                .unwrap_or(30_000);
            let dir = flag(args, "--dir")
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::temp_dir().join("mnemosyne-sources-bench"));
            run_bench(count, &dir)?;
        }
        _ => usage(),
    }
    Ok(())
}

fn load_plan(path: Option<&String>) -> photo_sources::Result<CommitPlan> {
    let path = path
        .map(PathBuf::from)
        .ok_or_else(|| photo_sources::Error::Message("plan json path required".into()))?;
    let text = fs::read_to_string(&path).map_err(|e| photo_sources::error::io_at(&path, e))?;
    Ok(serde_json::from_str(&text)?)
}

fn run_bench(count: usize, dir: &Path) -> photo_sources::Result<()> {
    if dir.exists() {
        let _ = fs::remove_dir_all(dir);
    }
    fs::create_dir_all(dir).map_err(|e| photo_sources::error::io_at(dir, e))?;
    let photos = dir.join("library");
    fs::create_dir_all(photos.join("Takeout/Google Photos"))
        .map_err(|e| photo_sources::error::io_at(&photos, e))?;
    let started_gen = Instant::now();
    for i in 0..count {
        let folder = if i % 20 == 0 {
            photos.join("Takeout/Google Photos")
        } else if i % 17 == 0 {
            photos.join("DCIM/Camera")
        } else {
            photos.join("inbox")
        };
        fs::create_dir_all(&folder).map_err(|e| photo_sources::error::io_at(&folder, e))?;
        let name = match i % 10 {
            0 => format!("Screenshot_{i:05}.png"),
            1 => format!("clip_{i:05}.mp4"),
            2 => format!("IMG_{i:05}.jpg.json"),
            _ => format!("IMG_{i:05}.jpg"),
        };
        let path = folder.join(&name);
        // Unique tiny payload so later commit-hash tests stay honest if reused.
        fs::write(&path, format!("m{i}\n").as_bytes()).map_err(|e| photo_sources::error::io_at(&path, e))?;
        if name.ends_with(".jpg") && i % 20 == 0 {
            let side = folder.join(format!("{name}.json"));
            fs::write(&side, b"{\"photoTakenTime\":{\"timestamp\":\"1700000000\"}}")
                .map_err(|e| photo_sources::error::io_at(&side, e))?;
        }
    }
    let gen_ms = started_gen.elapsed().as_millis();

    let before = rss_kb();
    let t0 = Instant::now();
    let first = enumerate_path(&photos, &PassThroughResolver)?;
    let first_ms = t0.elapsed().as_millis();
    let after_first = rss_kb();
    let media_count = first.media_count;
    let sidecar_count = first.sidecar_count;
    let media: Vec<_> = first
        .files
        .iter()
        .filter(|f| f.role == photo_sources::FileRole::Media)
        .take(50)
        .cloned()
        .collect();
    drop(first);
    let t1 = Instant::now();
    let second = enumerate_path(&photos, &PassThroughResolver)?;
    let second_ms = t1.elapsed().as_millis();
    let after_second = rss_kb();
    let peak = hwm_kb();
    drop(second);

    // A small album-only commit over the first 100 media ids, plus a copy of 50 files.
    let dest = dir.join("organized");
    fs::create_dir_all(&dest).map_err(|e| photo_sources::error::io_at(&dest, e))?;
    let store = dir.join("albums.json");
    let journal = dir.join("commit.json");
    let mut ops: Vec<PlanOp> = media
        .iter()
        .map(|f| PlanOp::AlbumAdd {
            item_id: f.id.clone(),
            album_id: "a_bench".into(),
            album_title: "Bench album".into(),
        })
        .collect();
    // copy mode is opt-in; use an explicit copy plan (not the product default).
    for f in media.iter().take(20) {
        ops.push(PlanOp::Copy {
            from: f.path.clone(),
            to: dest.join(&f.name),
        });
    }
    let plan = CommitPlan {
        id: "c_bench".into(),
        mode: CommitMode::Copy,
        confirmed: true,
        ops,
        album_store: Some(store.clone()),
    };
    let t2 = Instant::now();
    let dry = commit::dry_run(&plan)?;
    let applied = commit::apply(&plan, &journal)?;
    let apply_ms = t2.elapsed().as_millis();
    let t3 = Instant::now();
    commit::undo(&journal)?;
    let undo_ms = t3.elapsed().as_millis();

    let report = serde_json::json!({
        "library": photos,
        "generated_files_approx": count,
        "generate_ms": gen_ms,
        "first_enumerate_ms": first_ms,
        "rescan_enumerate_ms": second_ms,
        "media_count": media_count,
        "sidecar_count": sidecar_count,
        "rss_kb_before": before,
        "rss_kb_after_first": after_first,
        "rss_kb_after_rescan": after_second,
        "peak_rss_kb_hwm": peak,
        "commit_dry_ok": dry.ok,
        "commit_ops": applied.ops.len(),
        "commit_apply_ms": apply_ms,
        "commit_undo_ms": undo_ms,
        "note": "Enumeration only: no EXIF/date recovery or pairing (Theriz/Spin). Rescan is a second walk, not an incremental indexer."
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    let _ = io::stdout().flush();
    Ok(())
}

fn rss_kb() -> Option<u64> {
    let text = fs::read_to_string("/proc/self/status").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.split_whitespace().next()?.parse().ok();
        }
    }
    None
}

fn hwm_kb() -> Option<u64> {
    let text = fs::read_to_string("/proc/self/status").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            return rest.split_whitespace().next()?.parse().ok();
        }
    }
    None
}
