# photo_sources

Rust library and CLI for Mnemosyne **source adapters** and **journaled commit/undo**.

This crate locates folders, USB/removable drives (Windows-first) and social/Takeout exports, unpacks zips, and enumerates media plus sidecar files. It then calls a **format hook** (`FormatResolver`); on the web path the equivalent is Spin's `resolveMedia` in `web/formats/index.js`. The default hook is a pass-through and does **not** parse sidecar dates, pair Live/Motion Photos, or group bursts.

Cloud account APIs and direct MTP (Windows WPD) are documented stubs with **no network or device calls**.

Commit defaults to creating albums. Moving or renaming files is opt-in, requires `confirmed: true`, writes a journal, and can be fully undone.

See `docs/SOURCES-AND-COMMIT.md`, `docs/FORMAT-HOOK.md` and `docs/ITEM-CONTRACT.md` in the repo root.

```bash
cargo test
cargo run --bin photo_sources -- bench --count 30000 --dir /tmp/mnemosyne-sources-bench
```
