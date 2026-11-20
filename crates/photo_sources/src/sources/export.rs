//! Locate and unpack social / Takeout exports, then enumerate media and sidecar files.
//! Sidecar *contents* are not parsed here — Spin's [`crate::formats::FormatResolver`] owns that.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;
use zip::ZipArchive;

use crate::classify;
use crate::error::{self, Result};
use crate::file::{source_id_for, Enumeration, ExportKind, SourceKind};
use crate::formats::FormatResolver;
use crate::sources::{walk_enumeration, SourceAdapter};

pub struct ExportAdapter {
    /// Folder that will be walked (the unpacked tree, or a user-extracted folder).
    root: PathBuf,
    /// Original archive, if this adapter unpacked a zip.
    archive: Option<PathBuf>,
    kind: ExportKind,
    label: String,
    /// Temporary unpack directory to clean up later (optional).
    _staging: Option<PathBuf>,
}

impl ExportAdapter {
    pub fn from_folder(path: impl AsRef<Path>, kind: ExportKind) -> Self {
        let root = path.as_ref().to_path_buf();
        let label = format!("{} export", kind_label(kind));
        Self {
            root,
            archive: None,
            kind,
            label,
            _staging: None,
        }
    }

    pub fn from_archive(archive: impl AsRef<Path>) -> Result<Self> {
        let archive = archive.as_ref().to_path_buf();
        let staging = default_staging_dir(&archive)?;
        let report = unpack_export(&archive, &staging)?;
        let kind = report.export_kind;
        Ok(Self {
            root: report.root,
            archive: Some(archive),
            kind,
            label: format!("{} export", kind_label(kind)),
            _staging: Some(staging),
        })
    }

    pub fn export_kind(&self) -> ExportKind {
        self.kind
    }
}

impl SourceAdapter for ExportAdapter {
    fn kind(&self) -> SourceKind {
        SourceKind::Export
    }
    fn label(&self) -> &str {
        &self.label
    }
    fn root(&self) -> &Path {
        &self.root
    }
    fn enumerate(&self, resolver: &dyn FormatResolver) -> Result<Enumeration> {
        let root = std::fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone());
        walk_enumeration(
            &root,
            source_id_for(SourceKind::Export, self.archive.as_deref().unwrap_or(&root)),
            SourceKind::Export,
            Some(self.kind),
            self.label.clone(),
            None,
            Some(self.kind.as_str().to_string()),
            resolver,
        )
    }
}

fn kind_label(kind: ExportKind) -> &'static str {
    match kind {
        ExportKind::Takeout => "Google Takeout",
        ExportKind::Instagram => "Instagram",
        ExportKind::Facebook => "Facebook",
        ExportKind::Snapchat => "Snapchat",
        ExportKind::Tiktok => "TikTok",
        ExportKind::Unknown => "Social",
    }
}

fn default_staging_dir(archive: &Path) -> Result<PathBuf> {
    let parent = archive.parent().unwrap_or_else(|| Path::new("."));
    let stem = archive
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "export".into());
    let dir = parent.join(format!(".{stem}.mnemosyne-unpacked"));
    fs::create_dir_all(&dir).map_err(|e| error::io_at(&dir, e))?;
    Ok(dir)
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct UnpackReport {
    pub archive: PathBuf,
    pub root: PathBuf,
    pub export_kind: ExportKind,
    pub extracted: usize,
    pub skipped: usize,
}

/// Unpack a zip export, keeping media and sidecar entries only. Rejects zip-slip paths.
pub fn unpack_export(archive: &Path, dest: &Path) -> Result<UnpackReport> {
    fs::create_dir_all(dest).map_err(|e| error::io_at(dest, e))?;
    let file = File::open(archive).map_err(|e| error::io_at(archive, e))?;
    let mut zip = ZipArchive::new(file)?;
    let mut extracted = 0usize;
    let mut skipped = 0usize;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(rel) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            skipped += 1;
            continue;
        };
        if entry.is_dir() {
            let dir = dest.join(&rel);
            fs::create_dir_all(&dir).map_err(|e| error::io_at(&dir, e))?;
            continue;
        }
        let name = rel
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let keep = classify::role_of(&rel) != classify::Role::Other
            || classify::is_sidecar_name(&name)
            || is_export_manifest(&rel);
        if !keep {
            skipped += 1;
            continue;
        }
        let out_path = dest.join(&rel);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| error::io_at(parent, e))?;
        }
        let mut out = File::create(&out_path).map_err(|e| error::io_at(&out_path, e))?;
        io::copy(&mut entry, &mut out).map_err(|e| error::io_at(&out_path, e))?;
        extracted += 1;
    }
    let export_kind = detect_export_kind(dest).unwrap_or(ExportKind::Unknown);
    Ok(UnpackReport {
        archive: archive.to_path_buf(),
        root: dest.to_path_buf(),
        export_kind,
        extracted,
        skipped,
    })
}

fn is_export_manifest(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "archive_browser.html"
            | "index.html"
            | "memories_history.html"
            | "memories_history.json"
            | "user_data.json"
            | "posts_1.json"
            | "media.json"
            | "your_posts.json"
            | "metadata.json"
    ) || name.contains("memories_history")
}

/// Inspect a folder tree (and common filenames) to see which export it is.
/// Looks at names and locations only; does not parse JSON bodies.
pub fn detect_export_kind(root: &Path) -> Option<ExportKind> {
    let mut scores = [
        (ExportKind::Takeout, 0u32),
        (ExportKind::Instagram, 0),
        (ExportKind::Facebook, 0),
        (ExportKind::Snapchat, 0),
        (ExportKind::Tiktok, 0),
    ];
    let bump = |scores: &mut [(ExportKind, u32); 5], kind: ExportKind, n: u32| {
        if let Some((_, s)) = scores.iter_mut().find(|(k, _)| *k == kind) {
            *s += n;
        }
    };

    // Top-level and shallow markers.
    for entry in WalkDir::new(root)
        .follow_links(false)
        .max_depth(4)
        .into_iter()
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();

        if name == "takeout" || rel.starts_with("takeout/") || rel.contains("google photos/") {
            bump(&mut scores, ExportKind::Takeout, 5);
        }
        if name.ends_with(".supplemental-metadata.json") || name.ends_with(".supplemental-meta.json")
        {
            bump(&mut scores, ExportKind::Takeout, 8);
        }
        if name.ends_with(".jpg.json")
            || name.ends_with(".jpeg.json")
            || name.ends_with(".heic.json")
            || name.ends_with(".mp4.json")
        {
            bump(&mut scores, ExportKind::Takeout, 4);
        }
        if name == "metadata.json" && rel.contains("google") {
            bump(&mut scores, ExportKind::Takeout, 3);
        }

        if name.contains("instagram")
            || rel.contains("your_instagram_activity")
            || name == "posts_1.json"
            || name == "media.json"
            || rel.contains("/content/posts")
            || rel.contains("/media/posts")
        {
            bump(&mut scores, ExportKind::Instagram, 5);
        }
        if name.contains("facebook")
            || rel.contains("your_facebook_activity")
            || rel.contains("photos_and_videos")
            || name == "your_posts.json"
            || rel.contains("/your_photos")
        {
            bump(&mut scores, ExportKind::Facebook, 5);
        }
        if name.contains("snapchat")
            || name.contains("memories_history")
            || rel.contains("/memories/")
        {
            bump(&mut scores, ExportKind::Snapchat, 5);
        }
        if name.contains("tiktok") || name == "user_data.json" || rel.contains("tiktok") {
            bump(&mut scores, ExportKind::Tiktok, 5);
        }
    }

    scores.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
    let (kind, score) = scores[0];
    if score >= 4 {
        Some(kind)
    } else {
        None
    }
}

