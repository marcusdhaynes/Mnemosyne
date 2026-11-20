# Native index API

This is the contract for **Trice** (UI), **Anky** (sources and commit), **Spin** (formats), and **Angen** (faces and near-duplicates). The native indexer never moves, renames, or rewrites original files. Album assignments are proposals only.

The Rust commands live in `src-tauri/src/commands_index.rs`. `lib.rs` only adds those names to `generate_handler!`, so Anky's source/commit commands can merge beside them. Her command names are left untouched: `discover_removable_sources`, `enumerate_source`, `unpack_source_archive`, `list_cloud_stubs`, `connect_cloud_source`, `dry_run_commit`, `apply_commit`, `revert_commit`.

## JS bridge for Spin

Spin's `resolveMedia(files, {indexed})` (`web/formats/index.js`) takes `indexed` as a `Map` or object keyed by file path:

```js
{
  "/abs/path/IMG_0001.jpg": { takenAt: 1704067200000, gps: { lat: 38.72, lon: -9.14 } }
}
```

- `takenAt` — milliseconds since epoch, UTC, **only when the catalog has an EXIF/camera date**. Filename and mtime dates are omitted so Spin's sidecar and filename parsers can still fill gaps.
- `gps` — `{lat, lon}` from EXIF only, omitted when unknown.
- Native values are the **top-priority** date and GPS source (they win over sidecars).

```js
import {loadIndexed, withIndexed} from './native-index.js';
// In the Tauri shell:
const indexed = await loadIndexed(folderPath, window.__TAURI__.core.invoke);
const items = await resolveMedia([...mediaFiles, ...sidecarFiles], withIndexed({concurrency: 8}, indexed));
```

`loadIndexed` calls the `catalog_indexed` command and adds a forward-slash alias for Windows paths. Outside the native shell it returns `{}`. This module does not pair Live Photos, group bursts, extract Motion Photos, or parse export sidecars.

## Tauri commands

Existing commands, still in `lib.rs`:

| Command | Args | Result |
|---|---|---|
| `engine_version` | — | `"mnemosyne-indexer-0.2.0"` |
| `inspect_directory` | `{path}` | Fast uncapped listing (path, name, size, mtime, kind). No EXIF. For large libraries prefer `index_directory` + `catalog_page`. |
| `index_directory` | `{path}` | Incremental metadata index into the app-data SQLite catalog (`index.sqlite3`). Skips unchanged path + size + mtime. |
| `catalog_page` | `{path, offset, limit}` | Paged catalog rows (limit clamped to 512), newest capture date first. |

New commands, in `commands_index.rs`:

| Command | Args | Result |
|---|---|---|
| `generate_thumbnails` | `{path}` | Parallel 256px JPEG thumbs in the app-data `thumbs/` cache. Uses an embedded EXIF thumbnail when present. |
| `build_preview` | `{path}` | Pass-1 preview JSON (events, dates, types, proposed album assignments). |
| `write_preview` | `{path, out}` | Same preview, also written to `out`. |
| `catalog_stats` | `{path}` | Counts by `media_type`, plus `items` and `faces`. |
| `catalog_indexed` | `{path}` | The `indexed` object above, keyed by catalog path. |

The SQLite file is `{app_data_dir}/index.sqlite3`. The thumbnail cache is `{app_data_dir}/thumbs/{id[0:2]}/{id[2:4]}/{id}.jpg`.

CLI (no Tauri needed): `mnemosyne-index index\|thumbs\|preview\|bench <folder> --db PATH [--thumbs-dir PATH] [--out preview.json]`.

## Preview JSON

The pass-1 preview is compatible with Angen's `proposal.schema.json` so Trice's UI can read one shape. `scheme` defaults to `"event"`. `groups` is the event list (the same objects as `events`). `people` and `duplicates` are empty arrays for Angen to fill. `unsorted` is `{item_ids, reason_counts}` and starts empty.

`schemes`: `event`, `date`, `type`. (People grouping is Angen's.) Each group has `id`, `kind`, `title`, `confidence`, `reason`, `cover_item_id`, `item_ids`, `count`, `children`. The `events`, `dates` and `types` arrays remain so a caller can switch schemes without a recompute.

`proposals[]` (for Anky's commit dry-run):

```json
{ "item_id": "…", "path": "/abs/file.jpg", "action": "album_assign", "album": "Events/38.72°, -9.14° · Jun 3–6, 2024", "scheme": "event" }
```

Default commit mode should be albums only. The indexer never applies these.

## Catalog hooks for teammates

`indexed_media` columns the indexer fills: `path`, `item_id`, `size`, `modified_ms`, `kind`, `captured_ms`, `date_source` (`exif`\|`filename`\|`mtime`), `date_confidence`, `lat`, `lon`, `camera_make`, `camera_model`, `lens`, `width`, `height`, `duration_ms`, `media_type` (`photo`\|`screenshot`\|`selfie`\|`video`\|`short_vertical`\|`screen_recording`), `phash` (64-bit dHash), `thumb_path`, `front_camera`.

Columns the indexer **leaves NULL** for others to fill:

- `group_id` — Angen's near-duplicate / people grouping
- `pair_id` — Spin's Live Photo, Motion Photo, and burst pairing

Tables created empty for **Angen** (the indexer never writes them):

- `faces(id, item_path, bbox_x, bbox_y, bbox_w, bbox_h, landmarks_json, score, quality, embed_model, vec, person_id, confidence, created_ms)`
- `people(id, name, face_count, cover_face_id, confidence, pending_review)`

Key faces by `item_path` (the catalog primary key) or look up `item_id` (blake3 of the path, 16-byte hex). Re-indexing a file with the same path/size/mtime is a no-op and does not clear teammate columns.

## Type tags (pass 1)

- **screenshot** — filename (`Screenshot` / `Screen Shot`) or no camera make/model plus a screen-like size (and usually PNG).
- **selfie** — front-camera / FaceTime / selfie in make, model, or lens, or `selfie` in the filename.
- **short_vertical** — video with a ≥ 1.5 portrait aspect (9:16-ish), typically ≤ 180 s.
- **screen_recording** — video whose filename says so.
- **video** / **photo** — otherwise.

Bursts, Live Photos and Motion Photos are not grouped here.

## Crates (permissive licenses only)

| Crate | Role | License |
|---|---|---|
| jwalk, rayon | Parallel walk and CPU work | MIT / Apache-2.0 |
| kamadak-exif | EXIF date, GPS, camera, thumbnail | BSD-2-Clause |
| image (jpeg, png only) | Decode / 256px JPEG thumbs and dHash | MIT / Apache-2.0 |
| rusqlite (bundled) | WAL catalog | MIT |
| blake3 | Stable `item_id` and cache keys | CC0 / Apache-2.0 |
| serde, serde_json | Command and preview JSON | MIT / Apache-2.0 |
| tauri 2.11.6 | Desktop shell commands | Apache-2.0 / MIT |

ffmpeg (already on the machine) is used only to read non-MP4 duration and to grab a video keyframe. It is not bundled.
