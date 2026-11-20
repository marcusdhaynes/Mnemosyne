# Mnemosyne — Product Research, Feature Audit, and Truthful Parity Matrix

Updated 2026-11-19. This document describes the **reference designs**, **implemented features**, **partial features**, and **missing features**. It is not a claim of code-copying or feature parity. Product links are to official sources or repositories wherever possible.

## Ten reference applications (what users consistently value)

| Reference | Best-liked features to learn from | Mnemosyne status |
|---|---|---|
| Google Photos | Automated memories, people-and-pet grouping, intuitive natural-language search, photo stacks, almost-no-click browsing | Month timeline and rule-based sorting; real semantic search and reliable identity matching **not implemented** |
| Apple Photos | Calendar timeline, intelligent collections, on-device privacy, spatial / live photos, memories, hidden collections, elegant UI | Crystalline shell, timeline, favorites; live-photo pair handling and native PhotoKit **pending** |
| Adobe Lightroom Classic | Non-destructive photographic edits, RAW development, virtual copies, presets, quick culling and rating, batch metadata, XMP | Basic non-destructive brightness/contrast/saturation/rotation and JPEG export; RAW rendering, masks, curves, sidecars **pending** |
| digiKam | Huge local catalogs, face workflow, hierarchical tags, geolocation, duplicates, batch processing | SQLite native metadata catalog; flat tags, exact duplicates and bulk suggestion approvals; face embeddings and hierarchical tagging **pending** |
| Immich | Self-hosted originals, automatic backup, face clusters, CLIP semantic search, gallery map, companion sharing, RAW/video support | Local imports and video library; source adapters outlined; working backup, face embeddings and self-hosted sync **pending** |
| XnView MP | Excellent format coverage, fast browsing, contact sheet, comparison, file operations, batch conversion | Broad discovery of file types; browser preview limited by installed codecs; RAW/HEIC/JXL preview support requires native decoder |
| ACDSee Photo Studio | AI keywords, photo manager, on-device RAW workflow, quick background indexing | Filename/path tagging and native SQLite scanner; confidence-calibrated visual labels and RAW workflow **pending** |
| PhotoPrism | Privacy-first classification, similarity, maps, face recognition, local photo/video browsing | Source classification and review queue; neural embedding similarity, map and geographic clustering **pending** |
| Ente Photos | End-to-end encrypted sync and private on-device search and faces | Local browser catalog; **not end-to-end encrypted**; encryption and synchronization subject to independent security review |
| Photo Mechanic | Exceptionally rapid review/culling, keyboard-driven workflows, contact sheets, ingest from cameras | Keyboard search, batched imports, group-based review; full contact-sheet culling, select/reject and color labels **pending** |

Additional adjacent research: **darktable** for open-source RAW development, **Kdenlive / FFmpeg** for video export and editing, **CapCut** for native short-video timelines, and **Mylio** for offline device-spanning libraries. These are references, not dependencies.

## Every media source — supported vs integration-dependent

**Currently imported from user-selected files/folders/ZIP:** digital cameras, SD/CF/USB devices mounted as folders, phones' exported DCIM folders, downloads, screen captures, raw camera files as catalog entries, WhatsApp/Telegram downloads, Instagram Reels/Stories exports, TikTok videos, YouTube Shorts downloads or authorized exports, Snapchat Stories/Spotlight exports, Facebook Reels/Stories, X/Twitter video exports, CapCut-produced videos, Discord uploads, Reddit and Pinterest downloads, and generic desktop/mobile social exports. Actual media playback depends on browser codec support. Browser permissions do not allow unrestricted access to other apps' private storage.

**Common file types recognized for cataloging:** JPEG, PNG, WebP, GIF, TIFF, BMP, AVIF, HEIC/HEIF, JPEG XL, DNG, CR2/CR3, NEF, ARW, RAF, ORF, RW2, SRW and other camera RAW formats; MP4, M4V, MOV, WebM, 3GP/3G2, AVI, MKV, MPEG, WMV, FLV, MTS/M2TS, TS, ASF, HEVC, MXF and more. Catalog recognition is **not** a guarantee that the browser can preview or decode a file.

**Interactive cloud OAuth connections require additional credentials and permission review:** Google Drive through Google Picker and `drive.file`; Google Photos user-selected Photos Picker and approved exports, not unrestricted scanning; OneDrive via Microsoft Graph; Dropbox through its chooser API; Box; S3; WebDAV; Syncthing. Native Apple Photos/iCloud Photos uses PhotoKit with the user's approved permissions; Android uses Android Photo Picker and MediaStore. None of these account integrations is represented as connected in the web prototype.

**Copyright/account boundary:** Mnemosyne does not bypass DRM, access controls, private social APIs or platform terms of service. An authorized account export or downloaded file is a normal import source. Social links are not silently scraped.

## Reliability gates for identity recognition

Face detection (is there a face?) and identity recognition (who is it?) are separate tasks. The prototype has an **optional, browser-dependent, on-device face detector**, and users may manually name people. **It does not yet contain an advanced face-embedding classifier.** The browser API may be absent on most devices.

Target production design:
1. Detect and align faces with an explicitly licensed model (e.g., OpenCV YuNet; evaluate SFace and carefully check individual model license).
2. Generate embeddings offline via ONNX Runtime; cache by content hash + model version, delete cached biometric embeddings on request, never upload without opt-in.
3. Compare embeddings in an approximate nearest-neighbor vector index, cluster conservatively, and create **Unknown Person** groups for low-confidence matches.
4. Allow users to label one face once and review proposed matches in a side-by-side preview; never assign a real person's name without user-provided identity.
5. Calibrate quality against difficult cases: young vs old photos, lighting, occlusion, video frames, lookalikes, low resolution, group photos and non-Western demographics. Test false matches as well as recall. **Do not assert Google Photos-level performance without a benchmark.**
6. Offer small, balanced and maximum-accuracy models as optional downloads with clear memory, CPU/GPU and disk footprint controls.

## Sorting policy

- **Verified auto-assignment:** detected media type, unambiguous timestamps, explicit screenshot/screen-recording names, known source folders, video duration ≤ 180 s (any orientation).
- **Preliminary review:** keyword-inferred scenes, unverified face detections, future visual classifier predictions, unknown-duration short-video guesses, conflicting location/event assignments.
- **Human override:** approve or dismiss an entire group with one click; view individual pending photos in the Review view. Suggestions are non-destructive and original files are never moved by the sort engine.
- **Duplicates:** exact SHA-256 matches under 80 MB are skipped during import. For larger files and visual near-duplicates, full streaming hashes and an opt-in comparison workflow remain to be implemented. **No auto-delete of originals.**

## Performance budget and measurement

The interface uses 48-item incremental gallery pages, 16-file import database batches, short task yields, 640px preview thumbnails, and native SQLite streaming traversal. The duplicate-view filter uses a hash-frequency map rather than quadratic per-file comparisons.

The current automated benchmark verifies a 100,000-record duplicate-view **metadata filter**; it does **not** benchmark 100,000 image decodes, model inference, storage writes, or media rendering. Required measured targets before a production claim: 10,000 / 100,000 / 1,000,000-item library startup; import to first thumbnail; metadata-only scan of 50,000/250,000 files; background classification throughput on CPU/iGPU/dGPU; mobile storage/thermal and network sync; 99th-percentile search latency and crash recovery.

Browser IndexedDB still copies originals into app storage. Large libraries should use the native SQLite catalog and disk-backed thumbnail cache to avoid browser quota limits. There is currently **no benchmark-proven million-media implementation**.

## Legal integration list

- [libvips](https://github.com/libvips/libvips) — low-memory image engine, LGPL-2.1-or-later; check dynamic-linking compliance.
- [FFmpeg](https://github.com/FFmpeg/FFmpeg) — broad video codecs, LGPL/GPL varies by build and enabled options; audit redistributable binary licensing.
- [OpenCV Zoo](https://github.com/opencv/opencv_zoo) — YuNet and SFace face models; **check the individual model/weights licenses**, not only the source repository license.
- [Immich](https://github.com/immich-app/immich) — reference workflow architecture and benchmark expectations, AGPL-3.0; do not paste AGPL implementation into proprietary code without compliance.
- [Ente](https://github.com/ente-io/ente) — reference private mobile ML, AGPL-3.0; same integration caution.
- [darktable](https://github.com/darktable-org/darktable) — RAW development reference, GPL-3.0; inspiration, not a bundled source library.
- [Tauri v2 plugins](https://github.com/tauri-apps/plugins-workspace) — file/directory chooser, filesystem permissions, mobile privacy and platform bridge; integration needs signed mobile/desktop builds.

No third-party model weights or external library source has been copied into Mnemosyne in this iteration. Prefer a small, licensed native imaging library only when it demonstrably outperforms the platform decoder on representative test files.

## Non-negotiable next milestone

**High-accuracy optional AI engine**: real YuNet/SFace or audited commercially licensed model inference, embedding indexing, face cluster review, CLIP-style semantic search, and evidence-backed precision/recall comparisons; test and ship behind an opt-in toggle with local model downloader and opt-out deletion.

**Working account connections**: registered OAuth clients, native PhotoKit and Android MediaStore permissions, tested sync/revocation, E2EE requirements, and dedicated Supabase project instead of sharing a production database with another app.

**Professional media pipeline**: native libvips/LibRaw/FFmpeg preview and sidecar exports, stacked Live Photos/Motion Photos, RAW+JPEG pairing, burst detection, actual video trim/edit export, gallery culling, semantic search, recovery and undo.
