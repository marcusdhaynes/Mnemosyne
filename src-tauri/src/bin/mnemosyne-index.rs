//! CLI for indexing, thumbnails, preview, and the 50k benchmark.
//! Does not move or rewrite original files.
use mnemosyne_lib::{catalog, indexer, preview, thumbs};
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        print_help();
        return Ok(());
    }
    let cmd = args.remove(0);
    match cmd.as_str() {
        "index" => cmd_index(&args),
        "thumbs" => cmd_thumbs(&args),
        "preview" => cmd_preview(&args),
        "bench" => cmd_bench(&args),
        other => Err(format!("unknown command '{other}'")),
    }
}

fn print_help() {
    println!(
        "mnemosyne-index — fast metadata index, thumbs, and pass-1 preview\n\
         \n\
         Usage:\n\
           mnemosyne-index index   <folder> --db PATH [--thumbs-dir PATH] [--with-thumbs]\n\
           mnemosyne-index thumbs  <folder> --db PATH --thumbs-dir PATH\n\
           mnemosyne-index preview <folder> --db PATH [--out preview.json]\n\
           mnemosyne-index bench   <folder> --db PATH --thumbs-dir PATH [--out-dir PATH]\n\
         \n\
         Original files are never moved or rewritten."
    );
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn opt(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].clone())
}

fn positional_folder(args: &[String]) -> Result<String, String> {
    args.iter()
        .filter(|a| !a.starts_with('-'))
        .find(|_a| true)
        .cloned()
        .filter(|a| {
            let prev_is_flag = args
                .iter()
                .position(|x| x == a)
                .and_then(|i| if i == 0 { None } else { Some(&args[i - 1]) })
                .map(|p| p.starts_with("--") && p != "--with-thumbs")
                .unwrap_or(false);
            !prev_is_flag
        })
        .ok_or_else(|| "missing <folder>".into())
}

fn require_db(args: &[String]) -> Result<PathBuf, String> {
    opt(args, "--db")
        .map(PathBuf::from)
        .ok_or_else(|| "missing --db PATH".into())
}

fn cmd_index(args: &[String]) -> Result<(), String> {
    let folder = positional_folder(args)?;
    let db = require_db(args)?;
    let thumbs_dir = opt(args, "--thumbs-dir").map(PathBuf::from);
    let with = flag(args, "--with-thumbs");
    let started = Instant::now();
    let report = indexer::index_folder(&folder, &db, with, thumbs_dir.as_deref())?;
    println!(
        "indexed scanned={} changed={} unchanged={} skipped={} elapsed_ms={} (wall {} ms)",
        report.scanned,
        report.changed,
        report.unchanged,
        report.skipped,
        report.elapsed_ms,
        started.elapsed().as_millis()
    );
    Ok(())
}

fn cmd_thumbs(args: &[String]) -> Result<(), String> {
    let folder = positional_folder(args)?;
    let db = require_db(args)?;
    let cache = opt(args, "--thumbs-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| thumbs::cache_dir_next_to(&db));
    let report = thumbs::generate_thumbnails(&folder, &db, &cache)?;
    println!(
        "thumbs considered={} written={} from_exif={} decoded={} skipped={} failed={} elapsed_ms={}",
        report.considered,
        report.written,
        report.from_exif,
        report.decoded,
        report.skipped,
        report.failed,
        report.elapsed_ms
    );
    Ok(())
}

fn cmd_preview(args: &[String]) -> Result<(), String> {
    let folder = positional_folder(args)?;
    let db = require_db(args)?;
    let out = opt(args, "--out").map(PathBuf::from);
    let p = if let Some(path) = out {
        preview::write_preview_json(&folder, &db, &path)?
    } else {
        preview::build_preview(&folder, &db)?
    };
    println!(
        "preview items={} events={} date_groups={} type_groups={} proposals={} elapsed_ok",
        p.stats.items,
        p.events.len(),
        p.dates.len(),
        p.types.len(),
        p.proposals.len()
    );
    if opt(args, "--out").is_none() {
        let json = serde_json::to_string_pretty(&p).map_err(|e| e.to_string())?;
        io::stdout()
            .write_all(json.as_bytes())
            .map_err(|e| e.to_string())?;
        println!();
    }
    Ok(())
}

fn rss_kb() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            let n: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(n);
        }
    }
    None
}

fn cmd_bench(args: &[String]) -> Result<(), String> {
    let folder = positional_folder(args)?;
    let db = require_db(args)?;
    let cache = opt(args, "--thumbs-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| thumbs::cache_dir_next_to(&db));
    let out_dir = opt(args, "--out-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| db.parent().unwrap_or(Path::new(".")).to_path_buf());
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    if db.exists() {
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(db.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(Path::new(&format!("{}-wal", db.display())));
        let _ = std::fs::remove_file(Path::new(&format!("{}-shm", db.display())));
    }
    if cache.exists() {
        let _ = std::fs::remove_dir_all(&cache);
    }

    let cold_t = Instant::now();
    let cold = indexer::index_folder(&folder, &db, false, None)?;
    let cold_ms = cold_t.elapsed().as_millis();
    let cold_rss = rss_kb();

    let warm_t = Instant::now();
    let warm = indexer::index_folder(&folder, &db, false, None)?;
    let warm_ms = warm_t.elapsed().as_millis();
    let warm_rss = rss_kb();

    let th_t = Instant::now();
    let thumbs_r = thumbs::generate_thumbnails(&folder, &db, &cache)?;
    let th_ms = th_t.elapsed().as_millis();
    let th_rss = rss_kb();

    let pv_t = Instant::now();
    let preview_path = out_dir.join("preview.json");
    let pv = preview::write_preview_json(&folder, &db, &preview_path)?;
    let pv_ms = pv_t.elapsed().as_millis();
    let pv_rss = rss_kb();

    let n = cold.scanned.max(1);
    let result = serde_json::json!({
        "folder": folder,
        "scanned": cold.scanned,
        "cold": {
            "elapsed_ms": cold.elapsed_ms,
            "wall_ms": cold_ms,
            "changed": cold.changed,
            "unchanged": cold.unchanged,
            "skipped": cold.skipped,
            "per_1000_ms": (cold_ms as f64) * 1000.0 / (n as f64),
            "items_per_s": (n as f64) * 1000.0 / (cold_ms.max(1) as f64),
            "peak_rss_kb": cold_rss,
        },
        "warm": {
            "elapsed_ms": warm.elapsed_ms,
            "wall_ms": warm_ms,
            "changed": warm.changed,
            "unchanged": warm.unchanged,
            "per_1000_ms": (warm_ms as f64) * 1000.0 / (n as f64),
            "items_per_s": (n as f64) * 1000.0 / (warm_ms.max(1) as f64),
            "peak_rss_kb": warm_rss,
        },
        "thumbs": {
            "elapsed_ms": thumbs_r.elapsed_ms,
            "wall_ms": th_ms,
            "written": thumbs_r.written,
            "from_exif": thumbs_r.from_exif,
            "decoded": thumbs_r.decoded,
            "skipped": thumbs_r.skipped,
            "failed": thumbs_r.failed,
            "per_1000_ms": (th_ms as f64) * 1000.0 / (thumbs_r.considered.max(1) as f64),
            "thumbs_per_s": (thumbs_r.written.max(thumbs_r.considered) as f64) * 1000.0 / (th_ms.max(1) as f64),
            "peak_rss_kb": th_rss,
        },
        "preview": {
            "elapsed_ms": pv_ms,
            "items": pv.stats.items,
            "events": pv.events.len(),
            "date_groups": pv.dates.len(),
            "type_groups": pv.types.len(),
            "proposals": pv.proposals.len(),
            "peak_rss_kb": pv_rss,
        },
        "process_vmhwm_kb": rss_kb(),
    });
    let out = out_dir.join("bench.json");
    std::fs::write(&out, serde_json::to_string_pretty(&result).unwrap()).map_err(|e| e.to_string())?;
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    let _ = catalog::connect(&db)?;
    Ok(())
}
