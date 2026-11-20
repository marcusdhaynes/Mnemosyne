# Speed and size (measured)

Host: 8-core Intel Xeon at 2.40 GHz, 15 GiB RAM, no discrete GPU, Linux. Measured on Thursday, November 19, 2026, around 10:33 PM America/New_York with the `theriz/fast-index` release build (`mnemosyne-index`, `cargo build --release`).

The synthetic library is not in git. It was built from the 201 free photos in `/workspace/photo-app/engine/data/varied` and `data/commons` by `scripts/gen_bench_set.py`: 48,000 JPEGs with EXIF capture dates, GPS, camera tags and embedded thumbnails; 400 selfies with front-camera EXIF; 1,400 screenshot PNGs; 120 vertical short MP4s, 60 landscape videos and 20 screen recordings. **50,000 files total.** Originals were still 50,000 files after every run; the indexer never moves or rewrites them.

Times below are from one combined `mnemosyne-index bench` run (`bench.json`). Peak RAM for each stage is from a **fresh process** (Linux `VmHWM`), except cold index, which is the first stage of the combined run and so is also a fresh-process figure.

## Index, thumbnails, and preview (50,000 files)

| Stage | Wall time | Throughput | ms / 1,000 | Peak RAM (VmHWM) | Notes |
|---|---|---|---|---|---|
| Cold index (empty catalog) | 6.968 s | 7,176 items/s | 139.36 | 66,464 KiB (64.9 MiB) | 50,000 changed. EXIF dates and GPS on 48,400 stills; 1,400 screenshot dates from filenames; 200 video dates from mtime. |
| Warm index (unchanged files) | 0.186 s | 268,817 items/s | 3.72 | 66,880 KiB (65.3 MiB) | 50,000 skipped by path + size + mtime; 0 changed. Isolated re-run. |
| Thumbnails | 7.447 s | 6,714 thumbs/s | 148.94 | 156,140 KiB (152.5 MiB) | 48,400 written from embedded EXIF thumbnails; 1,600 decoded (PNG screenshots + video keyframes); 0 failed. Isolated re-run peak. Combined-run elapsed 7.336 s. |
| Pass-1 preview | 0.301 s | — | 6.02 | 147,248 KiB (143.8 MiB) | 1,242 events (time gap + place), 44 date groups, 6 type groups, 50,000 album-assignment proposals. Isolated re-run peak. |

A single process that ran all four stages in order reached a high-water mark of 260,640 KiB (254.5 MiB), because later stages stacked on earlier ones. The isolated figures in the table are the ones to use for capacity planning.

Catalog SQLite size after the cold index: 40 MiB. `preview.json` (pretty-printed, including every proposal) was 24 MiB.

## Release binary size (measured)

| Binary | Unstripped | Stripped (`strip`) |
|---|---|---|
| `mnemosyne` (Tauri app) | 13,585,872 bytes (12.96 MiB) | 10,826,672 bytes (10.33 MiB) |
| `mnemosyne-index` (CLI) | 4,859,544 bytes (4.63 MiB) | 4,149,968 bytes (3.96 MiB) |

These sizes are the indexer, thumbnailer, preview, and Tauri/CLI shell only. They do **not** include a face runtime or model files.

## Projected size with a face engine (not measured)

The face runtime is not chosen yet. Tyran asked Angen to try `tract` (pure Rust, a few MB) first and fall back to `ort` (~35 MB on Linux, Angen's estimate) only if `tract` cannot run YuNet and SFace or is clearly slower. Model files are about 10 MB (Angen's compressed SFace 9.9 MB + YuNet 0.2 MB). The totals below are **projections**, not measurements.

Starting from the measured unstripped `mnemosyne` app binary (12.96 MiB):

| Configuration | Runtime (projected) | Models (projected) | Projected total |
|---|---|---|---|
| App only (measured) | — | — | **12.96 MiB** (measured) |
| App + `tract` + models | a few MB (projected) | ~10 MB (projected) | **~24–29 MiB** (projection) |
| App + `ort` + models | ~35 MB on Linux (projected) | ~10 MB (projected) | **~59 MiB** (projection) |

Stripped equivalents would start from 10.33 MiB and add the same projected runtime and model sizes.

## Tests at the time of this measurement

- `cargo test` — 12 passed, 0 failed
- `npm test` — 22 passed, 0 failed
- `cargo build --release` — succeeded
