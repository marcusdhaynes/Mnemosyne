# Mnemosyne v0.2: Architecture and Safeguards

- **Web frontend** `web/`: plain ES modules, no mandatory JS framework runtime, network-independent for all basic features. Crystalline UI uses CSS-only glass surfaces, not heavy image assets.
- **Browser database** `web/db.js`: IndexedDB media and photo metadata, 16-file write transactions and 48-media thumbnail fetch batches. Browser quota may limit large libraries. Originals are never modified at the source; imported copies are stored in the browser origin.
- **Sorting** `web/intelligence.js`: deterministic trusted tags immediately applied, uncertain labels queued, explainable confidence and provenance. Does not imply actual trained scene classification or person recognition.
- **ZIP** `web/archives.js`: deflate and store-only decoder with path traversal, zip64, encryption, decompressed file size and ratio guards. Export metadata (`.json`/`.html` sidecars) is kept and interpreted by `web/formats/`.
- **Formats** `web/organizer.js`: wider file discovery than browser decoders. Unsupported original formats remain catalogued, with limited previews and lossless export of original bytes.
- **Media formats** `web/formats/`: Live Photo pairing (ContentIdentifier), Google/Samsung Motion Photo video offsets, burst stacks, original-date recovery from EXIF/QuickTime, Takeout/Instagram/Facebook/Snapchat sidecars and filenames, evidence-only short/screen-recording/slo-mo/time-lapse tags. See `MEDIA-FORMATS.md`.
- **Video** `web/media.js`: metadata probing for browser-decodable files, local frame thumbnail extraction when available, Shorts classification by duration ≤ 180 seconds regardless of aspect ratio.
- **Editor** `web/editor.js`: in-browser brightness, contrast, saturation, rotation; records a small edit recipe in the catalog rather than modifying original files; exports a JPEG copy up to a safe 6000px edge.
- **Face adapter** `web/recognition.js`: optional OS/Chromium FaceDetector when available, manual person naming and review of detected possible portraits; no training or identifying strangers by AI.
- **Native backend** `src-tauri/src/catalog.rs`: metadata-only recursive walk (no artificial 5000 file cap), SQLite WAL catalog with incremental upsert, bounded page queries. UI/native picker bridge remains to be wired and tested on each platform.
- **Cloud** `supabase/`: migration only; no automatic sync, no shared Somniel production database, and no cloud credentials in frontend. Every cloud connection requires explicit user approval and credential setup.

**Security constraints:** Do not auto-delete or auto-move files. Do not silently upload biometrics or pictures. Reject unsafe ZIP paths and decompression bombs. Only user-supplied person names become identity labels. Avoid unreviewed AGPL or noncommercial model weight redistribution. Ensure accessible focus and reduced-motion variants.

**Known limits:** The web gallery relies on browser codecs and storage quotas; expensive visual and face models are not bundled; native code has not been validated against actual Android/iOS device permissions. Read `FEATURE_AUDIT.md` for the deliberately separated feature-parity targets.
