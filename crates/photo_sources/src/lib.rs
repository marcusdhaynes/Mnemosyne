//! Source adapters, export unpacking, and journaled commit/undo for Mnemosyne.
//!
//! This crate does **not** implement format handling. After enumerating media and
//! sidecar files it calls [`formats::FormatResolver`]. The default is a pass-through;
//! Spin implements the real resolver (Live/Motion pairing, bursts, sidecar dates,
//! WhatsApp filenames). It also does **not** replace `indexer.rs` — enumeration
//! yields paths (and sidecar associations) for Theriz's indexer.

pub mod classify;
pub mod commit;
pub mod error;
pub mod file;
pub mod formats;
pub mod sources;

pub use commit::{
    apply, dry_run, undo, AlbumStore, CommitMode, CommitPlan, DryRunReport, Journal, PlanOp,
};
pub use error::{Error, Result};
pub use file::{Enumeration, ExportKind, FileRole, SourceFile, SourceKind};
pub use formats::{resolve_formats, FormatResolver, PassThroughResolver};
pub use sources::{
    discover_devices, enumerate_path, enumerate_path_default, open_path, unpack_export, CloudKind,
    CloudStub, DiscoveredDevice, SourceAdapter,
};

/// Default commit mode for the product: create albums, leave files where they are.
pub const DEFAULT_COMMIT_MODE: CommitMode = CommitMode::AlbumOnly;
