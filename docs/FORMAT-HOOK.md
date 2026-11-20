# Format hook (for Spin)

Sources unpack exports and walk folders or mounted devices, then **enumerate media and sidecar files**. They do not recover original dates, pair Live/Motion Photos, or group bursts.

After enumeration, every path goes through a single hook:

- Rust: `photo_sources::formats::FormatResolver::resolve_formats`
- JavaScript: Spin's `resolveMedia(files, {indexed})` in `web/formats/index.js` (see `docs/MEDIA-FORMATS.md`). `expandImports` in `web/archives.js` returns `{files, sidecars}` to pass straight in.

In Rust, the default `PassThroughResolver` returns the list unchanged and never reads sidecar contents.

## What Spin should implement

- Pair Live Photos (HEIC/JPG + MOV) and Android Motion Photos, **keeping the video**
- Group bursts
- Recover original dates from JSON sidecars in Google Takeout, Instagram, Facebook and Snapchat exports
- Recover dates from WhatsApp filename patterns

## Input

Each `SourceFile` (see `docs/ITEM-CONTRACT.md`) already has:

- `path`, `name`, `bytes`, `mtime_ms`
- `role`: `media` or `sidecar`
- `media_kind`: `image` or `video` when known from the extension
- `export_kind`: `takeout` | `instagram` | `facebook` | `snapchat` | `tiktok` when the tree was recognized
- `sidecar_paths`: nearby sidecar files located by naming convention (`photo.jpg.json`, `photo.jpg.supplemental-metadata.json`, `photo.xmp`, …). **Bodies are not parsed.**

Put recovered dates, pair ids and burst ids in the optional `format` JSON field (or any extra fields you add) so Theriz's indexer and Angen's engine can consume them. Do not drop motion-video files from the list.

## Wiring

In the native shell, pass your resolver into `enumerate_path(path, &your_resolver)` instead of `PassThroughResolver`. On the web, nothing to replace: Spin's `resolveMedia` is already the implementation.
