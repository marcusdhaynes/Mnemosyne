# Media formats layer (`web/formats/`)

Groups the pieces of one moment into a single item, recovers original capture dates, and tags special media **only when the evidence is there**. Pure ES modules with no dependencies, so it runs the same way in the browser, in Node 22+ (tests, tooling) and in the Tauri webview. It's read-only and never modifies, moves or deletes a file.

| File | Role |
|---|---|
| `web/formats/index.js` | **Public API**: `resolveMedia`, `inspectFile`, `classifyFile`, `extractMotionVideo`, `dateFromFilename`, `fixMojibake` |
| `web/formats/dates.js` | Filename rules, Takeout / Instagram / Facebook / Snapchat parsers, date priority (`chooseDate`) |
| `web/formats/motion.js` | Google and Samsung Motion Photo detection and video extraction |
| `web/formats/exif.js` | Small EXIF/XMP reader (JPEG, HEIC/AVIF via `iloc`, PNG `eXIf`, WebP) plus the Apple MakerNote |
| `web/formats/quicktime.js` | MOV/MP4 `moov` reader: `mdta` keys, `mvhd`, `tkhd`, `stts` frame rate |
| `web/formats/bytes.js` | Input normalisation (`File`/`Blob`/descriptor), magic-byte sniffing, bounded reads |

## API

```js
import {resolveMedia,extractMotionVideo} from './formats/index.js';
const items = await resolveMedia(files, {indexed, concurrency: 8});
```

**`files`**: media files **and** any export sidecars (`.json`, `.html`) from the same import. Each entry can be:
- a `File`/`Blob` (path from `virtualPath`, `webkitRelativePath`, or `name`);
- `{path, size, lastModified?, read(start,end) => Promise<Uint8Array>}`, for native or Node callers;
- `{path, bytes: Uint8Array, lastModified?}`.

Paths are used as given, with `\` normalised to `/`. Sidecars are matched to media by folder and path, so pass the export's relative paths.

**`indexed`** (optional): a `Map` or object keyed by the same path, holding `{takenAt?: msUTC, gps?: {lat, lon}}` from the native indexer. When present, it is the **top-priority** date and GPS source.

**Returns** `MediaItem[]`, sorted by `primaryPath`. One item per logical moment:

```ts
{
  id: string,               // `${firstTag||mediaKind}:${primaryPath}`, stable for a given path
  primaryPath: string,      // still for Live/Motion, cover for bursts, -main for Snapchat
  pairedVideoPath: string|null,   // Live Photo MOV (kept, never dropped)
  overlayPath: string|null,       // Snapchat -overlay.png
  motionVideo: {offset,length,mime,source,presentationTimestampUs}|null, // embedded MP4 byte range
  members: string[]|null,   // burst frames in time order (incl. cover)
  coverPath: string|null,   // burst cover
  files: {path, role}[],    // role: primary | live-video | burst-frame | overlay | sidecar
  kind: 'live-photo'|'motion-photo'|'burst'|'screen-recording'|'screenshot'|'slo-mo'|'time-lapse'|'short'|'photo'|'video',
  mediaKind: 'image'|'video', container: 'jpeg'|'heic'|'png'|'webp'|'mp4'|'quicktime'|…,
  tags: string[],           // every evidence-backed subtype; kind is the first by priority
  burstId, burstGrouping: 'burst-uuid'|'filename'|'timing'|null,
  liveId, livePairing: 'content-identifier'|'basename-time'|null,
  takenAt: number|null,     // ms since epoch (UTC)
  takenAtLocal: string|null,// wall-clock 'YYYY-MM-DDTHH:MM:SS' when the source has one
  takenAtSource: 'indexer'|'exif'|'quicktime'|'xmp'|'sidecar:takeout'|'sidecar:instagram'|'sidecar:instagram-taken'|
                 'sidecar:facebook'|'sidecar:facebook-taken'|'sidecar:snapchat'|'quicktime-mvhd'|'filename:<rule>'|'mtime',
  takenAtConfidence: 'embedded'|'sidecar'|'filename'|'filesystem'|'none',
  takenAtPrecision: 'ms'|'second'|'day'|null,   // WhatsApp names are day-only
  dateCandidates: {source,takenAt}[],           // every date seen, for review UIs
  gps: {lat,lon,alt?,source}|null,              // indexer > Takeout geoData > Snapchat location
  platform: 'takeout'|'instagram'|'facebook'|'snapchat'|'whatsapp'|null,
  make, model, durationSec, width, height, fps,
  evidence: string[],       // why each grouping or tag was applied
  error: string|null        // non-fatal parse problem; the item is still returned
}
```

Other exports:
- `inspectFile(file)`: per-file facts (`contentIdentifier`, `burstUUID`, `motionVideo`, `video{durationSec,fps,width,height,keys}`, `embeddedDates`).
- `extractMotionVideo(file, item.motionVideo)`: returns a `Blob` of the embedded MP4. It's a zero-copy slice for `File`/`Blob`.
- `dateFromFilename(name)` and `fixMojibake(text)`.

### Date priority

`indexer` → `exif` → `quicktime` (`com.apple.quicktime.creationdate`) → `xmp` → `sidecar:*` → `quicktime-mvhd` → `filename:*` → `mtime`.

The MP4 `mvhd` creation time ranks **below** sidecars on purpose. Instagram, Facebook and Snapchat re-encode uploads, which resets `mvhd` to processing time. All other embedded dates beat sidecars. Dates before 1971, the `0000:00:00` placeholder and dates more than a day in the future are rejected.

## What is detected and how

| Format | Evidence used | Result |
|---|---|---|
| **Apple Live Photo** | ① Apple MakerNote tag `0x0011` ContentIdentifier in the HEIC/JPEG equals `com.apple.quicktime.content.identifier` in the MOV. This works even when the MOV was renamed (`IMG_1 (1).MOV`). ② Fallback: same folder and basename (a trailing ` (N)` is ignored), the clip is ≤ 6 s, and the best timestamps are within 5 s. Conflicting identifiers never pair. | One `live-photo` item. `pairedVideoPath` keeps the MOV. |
| **Google Motion Photo** | XMP `Camera:MotionPhoto=1` plus the `Container:Directory` video item `Item:Length`. Offset = file size minus the trailing item lengths. Legacy `GCamera:MicroVideo=1` + `MicroVideoOffset` is also handled. Each offset is checked for `ftyp`. | `motion-photo`, with `motionVideo.{offset,length}` |
| **Samsung Motion Photo** | Only when the file ends in the SEF trailer (`SEFT`) or the XMP flags it: finds `MotionPhoto_Data`, then the first `ftyp`, and walks the MP4 boxes up to `SEFH`. | Same as above (`source:'samsung-sef'`) |
| **Burst** | ① Apple MakerNote `0x000b` BurstUUID. ② Filenames: `…BURST<17 digits>[_COVER]` (Pixel), `…_BURST001[_COVER]` (Huawei, Sony, generic), and 2 or more `YYYYMMDD_HHMMSS_NNN` (Samsung). ③ Timing: 3 or more frames from the same make, model and folder, ≤ 900 ms apart, using EXIF sub-second times only. | One `burst` item with `members` and `burstId`. The cover is the `COVER` frame, otherwise the earliest. |
| **Google Takeout** | The `*.json` / `*.supplemental-metadata*.json` sidecar in the same folder, matched by its `title`, then the exact name. Handles the `IMG(1).jpg` ↔ `IMG.jpg(1).json` counter swap, names truncated at 46 to 51 characters, and `-edited` copies. Uses `photoTakenTime.timestamp` and `geoData`/`geoDataExif`. | `sidecar:takeout` |
| **Instagram / Facebook** | Walks any export JSON for `uri` + `creation_timestamp` (upload time). It prefers `media_metadata…exif_data[].taken_timestamp` when present. Every string is passed through `fixMojibake` (Meta writes UTF-8 bytes as `\u00XX`). Matched by the `uri` path suffix, then by a unique basename. | `sidecar:instagram`, `sidecar:facebook`, or `…-taken` |
| **Snapchat Memories** | `memories_history.json` (`Saved Media[].Date` "… UTC", `Media Type`, `Location`, `mid`/`sid` from the download link) or the `.html` table. Matched by mid/sid in the filename, then by a `YYYY-MM-DD_HH-MM-SS` prefix, then by a single entry of the same type on that day. `-main` + `-overlay.png` become one item. | `sidecar:snapchat` |
| **WhatsApp** | `IMG|VID|AUD|PTT|STK|DOC-YYYYMMDD-WA####` | `filename:whatsapp`, day precision, `platform:'whatsapp'` |
| **Screenshots** | `Screenshot_YYYYMMDD-HHMMSS…` (Android), `Screenshot/Screen Shot YYYY-MM-DD at H.MM.SS [AM/PM]` (macOS), `Screenshot YYYY-MM-DD HHMMSS` (Windows), or EXIF UserComment `Screenshot` (iOS) | `screenshot` |
| **Screen recordings** | `Screen_Recording_YYYYMMDD-HHMMSS…` (Android), `Screen Recording … at …` (macOS), `RPReplay_Final<10 digits>` (iOS) | `screen-recording` |
| **Slo-mo** | Android `com.android.capture.fps` ≥ 100 and well above the playback fps, or a video track at ≥ 100 fps from `stts` | `slo-mo` |
| **Time-lapse** | Android `com.android.capture.fps` well below the playback fps. Bare iPhone files have no reliable tag, so they aren't guessed (PhotoKit can add this later). | `time-lapse` |
| **Short** | A video whose `mvhd` duration is ≤ 180 s. This matches `isShortVideo` in `intelligence.js`. Screen recordings are excluded. | `short` |

## Integration in the app

- `web/archives.js`: `expandZip`/`expandImports` now also return `sidecars` (`.json`/`.html` ≤ 20 MB) instead of dropping them.
- `web/main.js`: the import calls `resolveMedia` once per batch. Each photo record gets `takenAt`, `takenAtSource`, `mediaGroupId`, `mediaRole`, `formatKind`, `formatTags` and `motionVideo`.
- `web/organizer.js`: `visibleItems` hides companion files (`COMPANION_ROLES`: live video, extra burst frames, overlays) from tiles. They stay stored and still appear in Trash.
- `web/media.js`: `capturedAt` now reads HEIC, PNG, WebP and MOV/MP4 dates as well as JPEG, then falls back to filename patterns, then mtime.

## Not verified against real devices

The fixtures are synthetic and built to each published structure. These still need real sample files:
- Apple MakerNote byte layout on current iOS
- Samsung SEF variants
- Snapchat's current export filenames
- `RPReplay_Final` digits being Unix seconds
- Takeout truncation lengths
- iOS screenshot UserComment

The ExifTool tag tables (Apple `0x000b`/`0x0011`, MakerNote base offset +14) and the Android Motion Photo 1.0 spec were checked.
