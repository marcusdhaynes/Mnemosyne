use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::classify::{self, Role};
use crate::error::Result;
use crate::file::{self, Enumeration, ExportKind, SourceFile, SourceKind};
use crate::formats::{FormatResolver, PassThroughResolver};

pub mod cloud;
pub mod device;
pub mod export;
pub mod local;

pub use cloud::{CloudKind, CloudStub};
pub use device::{discover_devices, DiscoveredDevice, RemovableDeviceAdapter};
pub use export::{detect_export_kind, unpack_export, ExportAdapter, UnpackReport};
pub use local::LocalFolderAdapter;

pub trait SourceAdapter {
    fn kind(&self) -> SourceKind;
    fn label(&self) -> &str;
    fn root(&self) -> &Path;
    fn enumerate(&self, resolver: &dyn FormatResolver) -> Result<Enumeration>;
}

/// Auto-detect a local path: ZIP export, export folder, DCIM device, or ordinary folder.
pub fn open_path(path: impl AsRef<Path>) -> Result<Box<dyn SourceAdapter>> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(crate::error::Error::Message(format!(
            "path not found: {}",
            path.display()
        )));
    }
    if path.is_file() {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .eq_ignore_ascii_case("zip");
        if ext {
            return Ok(Box::new(ExportAdapter::from_archive(path)?));
        }
        return Err(crate::error::Error::Message(
            "expected a folder, a mounted device, or a .zip export".into(),
        ));
    }
    if let Some(kind) = detect_export_kind(path) {
        return Ok(Box::new(ExportAdapter::from_folder(path, kind)));
    }
    if device::looks_like_device(path) {
        return Ok(Box::new(RemovableDeviceAdapter::new(path)));
    }
    Ok(Box::new(LocalFolderAdapter::new(path)))
}

pub fn enumerate_path(
    path: impl AsRef<Path>,
    resolver: &dyn FormatResolver,
) -> Result<Enumeration> {
    open_path(path)?.enumerate(resolver)
}

pub fn enumerate_path_default(path: impl AsRef<Path>) -> Result<Enumeration> {
    enumerate_path(path, &PassThroughResolver)
}

pub(crate) fn walk_enumeration(
    root: &Path,
    source_id: String,
    source_kind: SourceKind,
    export_kind: Option<ExportKind>,
    label: String,
    device_kind: Option<String>,
    layout_hint: Option<String>,
    resolver: &dyn FormatResolver,
) -> Result<Enumeration> {
    let mut media = Vec::new();
    let mut sidecars = Vec::new();
    let mut skipped_other = 0u64;
    for entry in WalkDir::new(root).follow_links(false).into_iter() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                skipped_other += 1;
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        match file::from_path(entry.path(), &source_id, source_kind, export_kind, root) {
            Ok(None) => skipped_other += 1,
            Ok(Some((Role::Media, rec))) => media.push(rec),
            Ok(Some((Role::Sidecar, rec))) => sidecars.push(rec),
            Ok(Some((Role::Other, _))) => skipped_other += 1,
            Err(_) => skipped_other += 1,
        }
    }
    let mut sidecar_index: std::collections::HashMap<(PathBuf, String), PathBuf> =
        std::collections::HashMap::with_capacity(sidecars.len());
    for side in &sidecars {
        if let (Some(parent), Some(name)) = (side.path.parent(), side.path.file_name()) {
            sidecar_index.insert(
                (parent.to_path_buf(), name.to_string_lossy().to_ascii_lowercase()),
                side.path.clone(),
            );
        }
    }
    for item in media.iter_mut() {
        item.sidecar_paths = classify::sidecar_candidates_indexed(&item.path, &sidecar_index);
        if item.layout_hint.is_none() {
            item.layout_hint = layout_hint.clone();
        }
    }
    let sidecar_count = sidecars.len();
    let media_count = media.len();
    let mut files = media;
    files.extend(sidecars);
    let files = resolver.resolve_formats(files)?;
    Ok(Enumeration {
        source_id,
        source_kind,
        root: root.to_path_buf(),
        label,
        export_kind,
        device_kind,
        files,
        media_count,
        sidecar_count,
        skipped_other,
    })
}

/// Write an enumeration as NDJSON (one `SourceFile` per line) plus a header object if `header` is true.
pub fn write_ndjson(enumeration: &Enumeration, out: &mut dyn std::io::Write) -> Result<()> {
    for file in &enumeration.files {
        serde_json::to_writer(&mut *out, file)?;
        out.write_all(b"\n")?;
    }
    Ok(())
}

pub fn media_files(enumeration: &Enumeration) -> Vec<&SourceFile> {
    enumeration
        .files
        .iter()
        .filter(|f| f.role == file::FileRole::Media)
        .collect()
}
