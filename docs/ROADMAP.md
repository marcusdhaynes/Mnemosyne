# Roadmap / Feature Parity Matrix

The reference products are inspirations, not code dependencies or claims of parity.

| Reference | Sought-after capability | Current state | Next validation |
|---|---|---|---|
| Google Photos | Time travel, auto categories, semantic search, memories | Monthly view + filename rules | Local ONNX models, sampled accuracy tests |
| Apple Photos | Elegant curation, favorites, duplicates, hidden library | Favorites, exact duplicates | Private albums, native OS integrations |
| Lightroom | RAW, non-destructive edits, metadata workflows | Source preserved unchanged | Render pipeline, parameter sidecars, edit history |
| digiKam | Hierarchical tags, face recognition, bulk metadata | Flat tags + albums | Tree hierarchy, reviewable clustering, XMP |
| Immich | Local hosting, shared albums, device backup | Local storage only | Self-hosted server adapters, opt-in sync |
| XnView MP | Fast viewer, wide-format decoding, bulk tools | Browser-supported formats | libvips/LibRaw and batch processing |
| ACDSee | Metadata taxonomy, background indexing | Filename rules, Rust scanner skeleton | SQLite FTS5, native watcher, progress queue |
| PhotoPrism | Auto labels, geolocation, maps | Category inference via filename | EXIF geotags, maps, model confidence |
| Ente | E2EE sync, private ML | Offline first | Audited per-user keys, zero-knowledge sync |
| Amazon Photos | Shared family albums and backup | Multiple local albums | Scoped sharing and secure cloud backup |

## Milestones

**M1 — Reliable native library:** Native SQLite and filesystem scanning, file-path bookmarks and access scope, thumbnail disk cache, partial/changed file detection, camera EXIF extraction, corruption recovery, photo-bytes never copied into webview storage. Build and performance tests at 1K, 100K, 1M files.

**M2 — Intelligent organization:** Offline ONNX photo embeddings and labels; face grouping with explicit user review; semantic search; duplicate and near-duplicate review; review queue for confidence, never auto-delete files.

**M3 — Professional workflows:** RAW and HEIC/JXL preview codecs, sidecar XMP, non-destructive edit controls, compare, panorama, geolocation, advanced bulk metadata.

**M4 — Optional private cloud:** Dedicated Supabase deployment; tested user auth + RLS; opt-in encrypted upload; chunked background sync; conflict resolution; resumable uploads; recovery keys; user sharing. Deletion semantics and audit before shipping.

**M5 — All-platform release:** Windows/macOS/Linux native packages and signatures; Android/iOS native image-library access and permissions; tested background uploads; independent web/PWA edition; tagged releases, rollback plan, distribution channels.

**Exit criteria:** no unreviewed destructive file actions; no unauthorized cloud access; smoke, unit, integration, performance and adversarial tests; real device verification on every platform; successful and signed release artifacts. No subjective "perfect" claim absent that evidence.
