//! Format hook for Spin.
//!
//! After a source adapter unpacks an export or walks a folder/device, it enumerates
//! media and sidecar *paths* and then calls [`FormatResolver::resolve_formats`].
//!
//! Spin owns:
//! - Live Photo and Motion Photo pairing (video must be kept)
//! - burst grouping
//! - original-date recovery from Takeout / Instagram / Facebook / Snapchat JSON sidecars
//! - WhatsApp filename date patterns
//!
//! This crate ships only [`PassThroughResolver`], which returns the files unchanged
//! and never reads sidecar contents or groups pairs.

use crate::error::Result;
use crate::file::SourceFile;

pub trait FormatResolver {
    fn resolve_formats(&self, files: Vec<SourceFile>) -> Result<Vec<SourceFile>>;
}

/// Default hook. Spin should replace this in the app shell wiring with the real resolver.
#[derive(Debug, Default, Clone, Copy)]
pub struct PassThroughResolver;

impl FormatResolver for PassThroughResolver {
    fn resolve_formats(&self, files: Vec<SourceFile>) -> Result<Vec<SourceFile>> {
        Ok(files)
    }
}

/// Convenience entry point with a stable name for the JS hook (`resolveFormats`).
pub fn resolve_formats<R: FormatResolver + ?Sized>(
    resolver: &R,
    files: Vec<SourceFile>,
) -> Result<Vec<SourceFile>> {
    resolver.resolve_formats(files)
}
