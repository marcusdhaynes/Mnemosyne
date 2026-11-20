use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::classify::{self, MediaKind, Role};

/// Stable enumerated record handed to the indexer (Theriz) and to Spin's format hook.
/// This crate does not fill capture dates, GPS, pairs or bursts.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SourceFile {
    pub id: String,
    pub source_id: String,
    pub source_kind: SourceKind,
    pub path: PathBuf,
    pub name: String,
    pub bytes: u64,
    pub mtime_ms: u64,
    pub role: FileRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_kind: Option<MediaKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_kind: Option<ExportKind>,
    /// Companion sidecar paths located next to this file. Contents are not parsed here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sidecar_paths: Vec<PathBuf>,
    /// Relative path inside an export archive or folder, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub virtual_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_hint: Option<String>,
    /// Reserved for Spin: capture date, pairing, bursts, etc. Pass-through leaves this empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<serde_json::Value>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileRole {
    Media,
    Sidecar,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Folder,
    Device,
    Export,
    Cloud,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportKind {
    Takeout,
    Instagram,
    Facebook,
    Snapchat,
    Tiktok,
    Unknown,
}

impl ExportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ExportKind::Takeout => "takeout",
            ExportKind::Instagram => "instagram",
            ExportKind::Facebook => "facebook",
            ExportKind::Snapchat => "snapchat",
            ExportKind::Tiktok => "tiktok",
            ExportKind::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Enumeration {
    pub source_id: String,
    pub source_kind: SourceKind,
    pub root: PathBuf,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_kind: Option<ExportKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_kind: Option<String>,
    pub files: Vec<SourceFile>,
    pub media_count: usize,
    pub sidecar_count: usize,
    pub skipped_other: u64,
}

pub fn source_id_for(kind: SourceKind, root: &Path) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(format!("{kind:?}").as_bytes());
    hasher.update(b"\0");
    hasher.update(root.to_string_lossy().as_bytes());
    format!("s_{}", &hasher.finalize().to_hex()[..16])
}

pub fn file_id_for(path: &Path) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(path.to_string_lossy().as_bytes());
    format!("e_{}", &hasher.finalize().to_hex()[..16])
}

pub fn from_path(
    path: &Path,
    source_id: &str,
    source_kind: SourceKind,
    export_kind: Option<ExportKind>,
    root: &Path,
) -> crate::error::Result<Option<(Role, SourceFile)>> {
    let role = classify::role_of(path);
    if matches!(role, Role::Other) {
        return Ok(None);
    }
    let meta = std::fs::metadata(path).map_err(|e| crate::error::io_at(path, e))?;
    if !meta.is_file() {
        return Ok(None);
    }
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let virtual_path = path
        .strip_prefix(root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"));
    let file_role = match role {
        Role::Media => FileRole::Media,
        Role::Sidecar => FileRole::Sidecar,
        Role::Other => unreachable!(),
    };
    Ok(Some((
        role,
        SourceFile {
            id: file_id_for(path),
            source_id: source_id.to_string(),
            source_kind,
            path: path.to_path_buf(),
            name,
            bytes: meta.len(),
            mtime_ms,
            role: file_role,
            media_kind: classify::media_kind(path),
            export_kind,
            sidecar_paths: Vec::new(),
            virtual_path,
            layout_hint: None,
            format: None,
        },
    )))
}
