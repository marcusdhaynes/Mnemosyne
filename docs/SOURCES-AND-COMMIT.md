# Sources and commit (Anky)

Work lives in this clone on branch `anky/sources-commit`. The library crate is `crates/photo_sources`. Existing `web/sources.js` is extended, not replaced. `src-tauri/src/indexer.rs` and `catalog.rs` are untouched (Theriz).

## What works

- **Local folders** and **removable devices**: Windows drive-letter discovery (removable first), DCIM layouts for iPhone (`DCIM/100APPLE`), Android (`DCIM/Camera`) and cameras/SD cards. On Linux, `/media`, `/run/media` and `/mnt` plus any roots you pass in.
- **Export unpacking**: `.zip` extracts media and sidecar files only (zip-slip rejected). Folders are detected as Takeout, Instagram, Facebook, Snapchat or TikTok by layout and filenames.
- **Enumeration**: media + sidecar paths, including nearby `photo.jpg.json` associations **without reading the JSON**.
- **Formats**: on the web path, call Spin's `resolveMedia([...files, ...sidecars], {indexed})` from `web/formats/index.js` (see `docs/MEDIA-FORMATS.md`). In Rust, `FormatResolver` is the native hook; its default is a pass-through until the native side calls Spin's logic.
- **Cloud stubs** (no network): Google Drive, Google Photos (Takeout is the full-library route since March 2025), OneDrive, Dropbox, iCloud (synced folder or export; no public API).
- **MTP / WPD stub**: phones should use USB File transfer so they appear as a drive.
- **Commit**: default `album_only`. `copy`, `move` and `rename_only` are opt-in. `move` / `rename` require `confirmed: true`. Dry-run with conflict detection. Apply writes a journal. Undo is supported. Cross-device moves are copy, verify (blake3), then remove the source. User originals are never hard-deleted by album-only commits; undo of a copy only removes the file we created.

## What is stubbed

- All cloud OAuth APIs
- Direct MTP (Windows WPD / libmtp)
- Sidecar date parsing, Live/Motion pairing, bursts, WhatsApp filename dates (Spin)
- Incremental metadata index, EXIF, thumbnails (Theriz)

## How to run

```bash
cd crates/photo_sources
cargo test
cargo run --bin photo_sources -- discover ./fixtures
cargo run --bin photo_sources -- enumerate ./folder --ndjson /tmp/items.ndjson
cargo run --bin photo_sources -- unpack export.zip /tmp/unpacked
cargo run --bin photo_sources -- commit-dry plan.json
cargo run --bin photo_sources -- commit-apply plan.json journal.json
cargo run --bin photo_sources -- undo journal.json
cargo run --bin photo_sources -- bench --count 30000 --dir /tmp/mnemosyne-sources-bench
```

Web tests (includes the new adapters and album commit):

```bash
npm test
```

## Benchmark (this Linux box, 2026-11-19, release `photo_sources bench`)

Synthetic library of 30,000 tiny files (27,000 media + 3,000 sidecars) under `/tmp/mnemosyne-sources-bench`:

- First enumerate: **124 ms**
- Rescan enumerate: **119 ms** (a second full walk; incremental indexing is Theriz's)
- Peak RSS (VmHWM): **18,088 KB** (~18 MB)
- Commit apply (70 album + copy ops): **11 ms**
- Undo: **3 ms**

These numbers are for locating files only, not EXIF or format recovery.
