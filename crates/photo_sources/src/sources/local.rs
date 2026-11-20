use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::file::{source_id_for, Enumeration, SourceKind};
use crate::formats::FormatResolver;
use crate::sources::{walk_enumeration, SourceAdapter};

pub struct LocalFolderAdapter {
    root: PathBuf,
    label: String,
}

impl LocalFolderAdapter {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let root = path.as_ref().to_path_buf();
        let label = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned());
        Self { root, label }
    }
}

impl SourceAdapter for LocalFolderAdapter {
    fn kind(&self) -> SourceKind {
        SourceKind::Folder
    }
    fn label(&self) -> &str {
        &self.label
    }
    fn root(&self) -> &Path {
        &self.root
    }
    fn enumerate(&self, resolver: &dyn FormatResolver) -> Result<Enumeration> {
        let root = std::fs::canonicalize(&self.root)
            .or_else(|_| Ok::<PathBuf, std::io::Error>(self.root.clone()))
            .unwrap_or_else(|_| self.root.clone());
        walk_enumeration(
            &root,
            source_id_for(SourceKind::Folder, &root),
            SourceKind::Folder,
            None,
            self.label.clone(),
            None,
            None,
            resolver,
        )
    }
}
