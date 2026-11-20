# Item contract — enumerated source files

This is the record `photo_sources` emits for each file it locates. It is intentionally smaller than the engine's full item (capture date, GPS, fingerprints, people). Those fields are filled later by Theriz's indexer and Spin's format hook.

```json
{
  "id": "e_0123456789abcdef",
  "source_id": "s_0123456789abcdef",
  "source_kind": "folder",
  "path": "/media/phone/DCIM/Camera/IMG_0001.jpg",
  "name": "IMG_0001.jpg",
  "bytes": 2048,
  "mtime_ms": 1704067200000,
  "role": "media",
  "media_kind": "image",
  "export_kind": "takeout",
  "sidecar_paths": ["/media/phone/DCIM/Camera/IMG_0001.jpg.json"],
  "virtual_path": "DCIM/Camera/IMG_0001.jpg",
  "layout_hint": "dcim_android",
  "format": null
}
```

- `source_kind`: `folder` | `device` | `export` | `cloud`
- `role`: `media` | `sidecar`
- `media_kind`: `image` | `video` | omitted for sidecars
- `export_kind`: `takeout` | `instagram` | `facebook` | `snapchat` | `tiktok` | `unknown` | omitted
- `format`: reserved for Spin. The pass-through hook leaves it `null` / omitted.
- IDs are stable hashes of the source root or file path (`e_` / `s_` + 16 hex chars of blake3).

CLI: `photo_sources enumerate DIR --ndjson items.ndjson` writes one record per line.
