# Mnemosyne

> **Version 0.2.0 development:** Crystalline glass interface, staged review queue, source-oriented imports and social ZIP support, Shorts classification, non-destructive basic edits, and indexed native SQLite catalog. The advanced ML face-identification and connected cloud drives are **not yet available**; see [feature audit](docs/FEATURE_AUDIT.md) and [architecture](docs/ARCHITECTURE.md).


**Your private photo atelier.** A working, local-first photographic library with a carefully designed cross-platform foundation.

> **Status: v0.1.0 prototype, not a completed clone of 10 mature photo managers.** The web app is functional; desktop packages and mobile apps are not yet built or tested. Do not use the prototype as the sole copy of valuable photographs.

## Run it

Requires Node.js 22+. No package installation required to run the web app:

```bash
npm run dev
# Open http://localhost:5173
```

Check and generate static website files:

```bash
npm run check
npm run preview
```

To package the Tauri desktop shell, install the Rust toolchain and system dependencies, then run:

```bash
npm install
npm run tauri dev
npm run tauri build
```

Native Rust is unverified in this workspace because the Rust toolchain was unavailable. The web app is intentionally runnable without any framework package downloads. The Tauri wrapper uses the same functional web interface and includes an independent native metadata scanner that is not connected to the UI yet.

## What works in the prototype

- Import single/multiple photos or videos; import folders where browser file-picking supports it; drag and drop files.
- Store original files and generated WebP previews in local IndexedDB (no account or upload required).
- Lazy photo gallery, large and standard layouts, search by filename/folder/tags, sort and date/type filtering.
- Albums (including multiple memberships), favorites, manual tags and filename-based tag suggestions.
- Monthly moments view, basic Portraits view (keyword/manually tagged), slideshow and keyboard viewer navigation.
- Exact hash duplicate grouping (SHA-256 for files up to 80 MiB, intentionally skips larger files); 8x8 perceptual signatures are recorded but not yet exposed as a similarity tool.
- Soft delete and restore; export metadata catalog JSON (not a complete original-file backup).
- Responsive, custom-built editorial design with Tauri desktop shell configuration and automated web CI.

## Known constraints

- No face recognition, object detection, visual AI natural-language search, automated photo enhancement, geographic map, advanced editing or cross-device cloud synchronization **yet**.
- RAW and HEIC/HEIF/JPEG XL files can be imported but may not display previews unless the browser supports decoding. Format-specific native codecs are still required.
- Dates are derived from file modification timestamps, not camera EXIF capture dates.
- Browser storage is subject to quotas and eviction. A production native release should index original file paths without duplicating large libraries in IndexedDB.
- Native scanner currently returns metadata for at most 5,000 files per invocation, and does not yet persist a native catalog or ingest into the gallery.
- Mobile Tauri build generation, native permissions, backup, background indexing and app-store signing are not implemented.
- No end-to-end photo encryption or shared cloud album security has been implemented; **do not enable cloud uploads** before threat-model review and testing.

## Stack

- **Currently running**: JavaScript ES modules, semantic HTML, responsive CSS, browser IndexedDB, Web Crypto SHA-256, Canvas/WebP thumbnailing, Node 22 test and build scripts.
- **Native foundation**: Rust + Tauri 2, cross-platform source, metadata scanner, OS bundle configurations and GitHub Actions desktop release template.
- **Planned**: migration to Svelte 5 + TypeScript, native SQLite catalog and filesystem watcher, libvips/LibRaw/Exiv2 image pipeline, ONNX local ML inference, secure optional Supabase sync.

## Supabase

`supabase/migrations/20261119220000_initial_schema.sql` provisions an **independent project** with nine owner-isolated public tables, row-level security, and two private storage buckets. It is **not deployed or connected**. Never apply this migration to an existing Somniel/ProSeU project. Confirm new project organization, region and quoted cost first. Cloud is optional.

## GitHub Pages live preview

See [docs/DEPLOY.md](docs/DEPLOY.md) for the exact repository setup instructions. The app builds to `dist/` and is compatible with a GitHub Pages project path. The Pages workflow runs tests, validates built assets, archives the preview for recovery, checks that it is deploying the latest `main` revision, and publishes via GitHub Actions. The site is not publicly live until Pages is enabled and a deployment succeeds.

## Release policy

- `main` is the future stable branch. Every committed update is recoverable through Git history.
- PR/push validation runs web tests, generates a web preview, and builds/checks Rust where toolchains are available.
- GitHub Pages workflow publishes the web preview once enabled in repository settings.
- Pushing a `v*` tag triggers desktop builds and creates a **draft prerelease**, not an untested auto-published stable build.
- Android/iOS workflow targets are not enabled until native permissions, CI provisioning, signing, and device tests are implemented.

See `docs/ROADMAP.md`, `docs/SECURITY.md`, and `docs/DESIGN.md` for details.
