//! Journaled commit with dry-run, apply, and full undo.
//!
//! Default mode is [`CommitMode::AlbumOnly`]: nothing is moved or renamed.
//! `move` / `rename` are opt-in and require `confirmed: true`.
//! User originals are never hard-deleted. A cross-device move is copy, verify, then
//! remove the source only when the plan says move and verification succeeded.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{self, Error, Result};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommitMode {
    /// Default. Writes album membership only; original files stay put.
    AlbumOnly,
    Copy,
    Move,
    RenameOnly,
}

impl Default for CommitMode {
    fn default() -> Self {
        Self::AlbumOnly
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PlanOp {
    Mkdir { path: PathBuf },
    Copy { from: PathBuf, to: PathBuf },
    Move { from: PathBuf, to: PathBuf },
    Rename { from: PathBuf, to: PathBuf },
    AlbumAdd {
        item_id: String,
        album_id: String,
        album_title: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CommitPlan {
    pub id: String,
    pub mode: CommitMode,
    #[serde(default)]
    pub confirmed: bool,
    pub ops: Vec<PlanOp>,
    #[serde(default)]
    pub album_store: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Conflict {
    pub message: String,
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DryRunReport {
    pub plan_id: String,
    pub mode: CommitMode,
    pub ops: usize,
    pub bytes_to_copy: u64,
    pub mkdir_count: usize,
    pub file_ops: usize,
    pub album_ops: usize,
    pub conflicts: Vec<Conflict>,
    pub same_volume: bool,
    pub dest_free_bytes: Option<u64>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation_needed: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct JournalOp {
    pub seq: usize,
    pub op: PlanOp,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_copy: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Journal {
    pub commit_id: String,
    pub mode: CommitMode,
    pub started_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_ms: Option<u64>,
    pub path: PathBuf,
    pub album_store: Option<PathBuf>,
    pub ops: Vec<JournalOp>,
    pub undone: bool,
}

impl Journal {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).map_err(|e| error::io_at(path, e))?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| error::io_at(parent, e))?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self)?;
        fs::write(&tmp, text).map_err(|e| error::io_at(&tmp, e))?;
        fs::rename(&tmp, &self.path).map_err(|e| error::io_at(&self.path, e))?;
        Ok(())
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn new_commit_id() -> String {
    format!("c_{:016x}", now_ms().wrapping_mul(0x9e37_79b9) ^ std::process::id() as u64)
}

pub fn dry_run(plan: &CommitPlan) -> Result<DryRunReport> {
    let mut conflicts = Vec::new();
    let mut bytes_to_copy = 0u64;
    let mut mkdir_count = 0usize;
    let mut file_ops = 0usize;
    let mut album_ops = 0usize;
    let mut dests: HashSet<PathBuf> = HashSet::new();
    let mut same_volume = true;
    let mut dest_roots: Vec<PathBuf> = Vec::new();

    let confirmation_needed = match plan.mode {
        CommitMode::Move | CommitMode::RenameOnly if !plan.confirmed => {
            Some("move/rename requires confirmed=true".to_string())
        }
        _ => None,
    };

    if matches!(plan.mode, CommitMode::AlbumOnly) {
        for op in &plan.ops {
            if !matches!(op, PlanOp::AlbumAdd { .. }) {
                conflicts.push(Conflict {
                    message: "album_only plans may only contain album_add operations".into(),
                    path: None,
                });
                break;
            }
        }
    }

    for op in &plan.ops {
        match op {
            PlanOp::Mkdir { path } => {
                mkdir_count += 1;
                dest_roots.push(path.clone());
            }
            PlanOp::Copy { from, to } | PlanOp::Move { from, to } | PlanOp::Rename { from, to } => {
                file_ops += 1;
                if !from.is_file() {
                    conflicts.push(Conflict {
                        message: format!("source missing: {}", from.display()),
                        path: Some(from.clone()),
                    });
                    continue;
                }
                if matches!(op, PlanOp::Rename { .. }) && from.parent() != to.parent() {
                    conflicts.push(Conflict {
                        message: "rename_only destinations must stay in the same folder".into(),
                        path: Some(to.clone()),
                    });
                }
                if matches!(plan.mode, CommitMode::RenameOnly) && matches!(op, PlanOp::Copy { .. } | PlanOp::Move { .. }) {
                    conflicts.push(Conflict {
                        message: "rename_only plans may not copy or move across folders".into(),
                        path: Some(to.clone()),
                    });
                }
                if matches!(plan.mode, CommitMode::Copy) && matches!(op, PlanOp::Move { .. }) {
                    conflicts.push(Conflict {
                        message: "copy mode plan contains a move".into(),
                        path: Some(from.clone()),
                    });
                }
                let meta = fs::metadata(from).map_err(|e| error::io_at(from, e))?;
                bytes_to_copy += meta.len();
                if !dests.insert(to.clone()) {
                    conflicts.push(Conflict {
                        message: format!("two operations write to {}", to.display()),
                        path: Some(to.clone()),
                    });
                }
                if to.exists() {
                    if files_identical(from, to)? {
                        // idempotent — not a conflict
                    } else {
                        conflicts.push(Conflict {
                            message: format!("destination exists: {}", to.display()),
                            path: Some(to.clone()),
                        });
                    }
                }
                if let (Some(a), Some(b)) = (volume_id(from), volume_id(to.parent().unwrap_or(to))) {
                    if a != b {
                        same_volume = false;
                    }
                }
                dest_roots.push(to.parent().unwrap_or(to).to_path_buf());
            }
            PlanOp::AlbumAdd { .. } => album_ops += 1,
        }
    }

    let dest_free_bytes = dest_roots.iter().find_map(|p| available_bytes(p));
    if let Some(free) = dest_free_bytes {
        if bytes_to_copy > 0 && free < bytes_to_copy {
            conflicts.push(Conflict {
                message: format!("not enough free space: need {bytes_to_copy} bytes, have {free}"),
                path: dest_roots.first().cloned(),
            });
        }
    }

    let ok = conflicts.is_empty() && confirmation_needed.is_none();
    Ok(DryRunReport {
        plan_id: plan.id.clone(),
        mode: plan.mode,
        ops: plan.ops.len(),
        bytes_to_copy,
        mkdir_count,
        file_ops,
        album_ops,
        conflicts,
        same_volume,
        dest_free_bytes,
        ok,
        confirmation_needed,
    })
}

pub fn apply(plan: &CommitPlan, journal_path: &Path) -> Result<Journal> {
    let report = dry_run(plan)?;
    if report.confirmation_needed.is_some() {
        return Err(Error::ConfirmationRequired("move/rename"));
    }
    if !report.ok {
        let detail = report
            .conflicts
            .iter()
            .map(|c| c.message.clone())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(Error::Conflict(detail));
    }

    let mut journal = Journal {
        commit_id: if plan.id.is_empty() {
            new_commit_id()
        } else {
            plan.id.clone()
        },
        mode: plan.mode,
        started_ms: now_ms(),
        finished_ms: None,
        path: journal_path.to_path_buf(),
        album_store: plan.album_store.clone(),
        ops: plan
            .ops
            .iter()
            .enumerate()
            .map(|(seq, op)| JournalOp {
                seq,
                op: op.clone(),
                done: false,
                hash: None,
                created: None,
                bytes: None,
                used_copy: None,
            })
            .collect(),
        undone: false,
    };
    journal.save()?;

    let n = journal.ops.len();
    for i in 0..n {
        apply_one(&mut journal.ops[i], plan.album_store.as_deref())?;
        journal.save()?;
    }
    journal.finished_ms = Some(now_ms());
    journal.save()?;
    Ok(journal)
}

// Fix ConfirmationRequired - I'll change the check to Error::Message to avoid leak.
fn apply_one(slot: &mut JournalOp, album_store: Option<&Path>) -> Result<()> {
    match &slot.op {
        PlanOp::Mkdir { path } => {
            let created = !path.exists();
            fs::create_dir_all(path).map_err(|e| error::io_at(path, e))?;
            slot.created = Some(created);
            slot.done = true;
        }
        PlanOp::Copy { from, to } => {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| error::io_at(parent, e))?;
            }
            let (hash, bytes, created) = copy_verify(from, to)?;
            slot.hash = Some(hash);
            slot.bytes = Some(bytes);
            slot.created = Some(created);
            slot.used_copy = Some(true);
            slot.done = true;
        }
        PlanOp::Move { from, to } | PlanOp::Rename { from, to } => {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| error::io_at(parent, e))?;
            }
            let same = same_volume(from, to);
            if same && !to.exists() {
                fs::rename(from, to).map_err(|e| error::io_at(to, e))?;
                slot.used_copy = Some(false);
                slot.hash = Some(hash_file(to)?);
                slot.bytes = Some(fs::metadata(to).map(|m| m.len()).unwrap_or(0));
                slot.created = Some(true);
                slot.done = true;
            } else {
                let (hash, bytes, created) = copy_verify(from, to)?;
                // Only remove the source after the destination hash matches.
                if hash_file(to)? != hash {
                    return Err(Error::Message(format!(
                        "hash mismatch after copy to {}, leaving source in place",
                        to.display()
                    )));
                }
                fs::remove_file(from).map_err(|e| error::io_at(from, e))?;
                slot.used_copy = Some(true);
                slot.hash = Some(hash);
                slot.bytes = Some(bytes);
                slot.created = Some(created);
                slot.done = true;
            }
        }
        PlanOp::AlbumAdd {
            item_id,
            album_id,
            album_title,
        } => {
            if let Some(store) = album_store {
                album_add(store, album_id, album_title, item_id)?;
            }
            slot.done = true;
        }
    }
    Ok(())
}

pub fn undo(journal_path: &Path) -> Result<Journal> {
    let mut journal = Journal::load(journal_path)?;
    if journal.undone {
        return Err(Error::Message("this commit has already been undone".into()));
    }
    for slot in journal.ops.iter_mut().rev() {
        if !slot.done {
            continue;
        }
        undo_one(slot, journal.album_store.as_deref())?;
        slot.done = false;
    }
    journal.undone = true;
    journal.finished_ms = Some(now_ms());
    journal.save()?;
    Ok(journal)
}

fn undo_one(slot: &mut JournalOp, album_store: Option<&Path>) -> Result<()> {
    match &slot.op {
        PlanOp::Mkdir { path } => {
            if slot.created == Some(true) {
                // Only remove directories we created, and only if they are empty.
                let _ = fs::remove_dir(path);
            }
        }
        PlanOp::Copy { to, .. } => {
            // Remove only the file we created, after verifying it is still ours.
            if slot.created == Some(true) && to.is_file() {
                if let Some(expected) = &slot.hash {
                    if hash_file(to)? == *expected {
                        fs::remove_file(to).map_err(|e| error::io_at(to, e))?;
                    }
                }
            }
        }
        PlanOp::Move { from, to } | PlanOp::Rename { from, to } => {
            if to.is_file() {
                if let Some(parent) = from.parent() {
                    fs::create_dir_all(parent).map_err(|e| error::io_at(parent, e))?;
                }
                if same_volume(to, from) && !from.exists() {
                    fs::rename(to, from).map_err(|e| error::io_at(from, e))?;
                } else {
                    let (_hash, _, _) = copy_verify(to, from)?;
                    if let Some(expected) = &slot.hash {
                        if hash_file(to)? == *expected {
                            fs::remove_file(to).map_err(|e| error::io_at(to, e))?;
                        }
                    } else {
                        fs::remove_file(to).map_err(|e| error::io_at(to, e))?;
                    }
                }
            }
        }
        PlanOp::AlbumAdd {
            item_id, album_id, ..
        } => {
            if let Some(store) = album_store {
                album_remove(store, album_id, item_id)?;
            }
        }
    }
    Ok(())
}

fn copy_verify(from: &Path, to: &Path) -> Result<(String, u64, bool)> {
    let created = !to.exists();
    if to.exists() && files_identical(from, to)? {
        let meta = fs::metadata(to).map_err(|e| error::io_at(to, e))?;
        return Ok((hash_file(to)?, meta.len(), false));
    }
    if to.exists() {
        return Err(Error::Conflict(format!(
            "refusing to overwrite {}",
            to.display()
        )));
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| error::io_at(parent, e))?;
    }
    // Copy to a sibling temp, then atomically rename into place.
    let tmp = tmp_beside(to);
    fs::copy(from, &tmp).map_err(|e| error::io_at(&tmp, e))?;
    let hash = hash_file(&tmp)?;
    let src_hash = hash_file(from)?;
    if hash != src_hash {
        let _ = fs::remove_file(&tmp);
        return Err(Error::Message(format!(
            "copy verification failed for {}",
            to.display()
        )));
    }
    fs::rename(&tmp, to).map_err(|e| error::io_at(to, e))?;
    let bytes = fs::metadata(to).map_err(|e| error::io_at(to, e))?.len();
    Ok((hash, bytes, created))
}

fn tmp_beside(to: &Path) -> PathBuf {
    let name = to
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    to.with_file_name(format!(".{name}.mnemosyne-tmp-{}", std::process::id()))
}

pub fn hash_file(path: &Path) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut file = File::open(path).map_err(|e| error::io_at(path, e))?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| error::io_at(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn files_identical(a: &Path, b: &Path) -> Result<bool> {
    let ma = fs::metadata(a).map_err(|e| error::io_at(a, e))?;
    let mb = fs::metadata(b).map_err(|e| error::io_at(b, e))?;
    if ma.len() != mb.len() {
        return Ok(false);
    }
    Ok(hash_file(a)? == hash_file(b)?)
}

fn same_volume(from: &Path, to: &Path) -> bool {
    let dest_probe = if to.exists() {
        to
    } else {
        to.parent().unwrap_or(to)
    };
    match (volume_id(from), volume_id(dest_probe)) {
        (Some(a), Some(b)) => a == b,
        _ => true, // assume same volume and let rename fail over to copy if needed
    }
}

fn volume_id(path: &Path) -> Option<u64> {
    let probe = if path.exists() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        return fs::metadata(probe).ok().map(|m| m.dev());
    }
    #[cfg(windows)]
    {
        return windows_volume_serial(&probe);
    }
    #[allow(unreachable_code)]
    None
}

pub fn available_bytes(path: &Path) -> Option<u64> {
    let probe = if path.exists() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| path.to_path_buf())
    };
    #[cfg(unix)]
    {
        let cstr = std::ffi::CString::new(probe.to_string_lossy().as_bytes()).ok()?;
        let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
        let rc = unsafe { libc::statvfs(cstr.as_ptr(), &mut s) };
        if rc == 0 {
            return Some(s.f_bavail as u64 * s.f_frsize as u64);
        }
        return None;
    }
    #[cfg(windows)]
    {
        return windows_free_space(&probe);
    }
    #[allow(unreachable_code)]
    None
}

// --- album store (JSON file) ------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AlbumStore {
    pub albums: HashMap<String, AlbumRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumRecord {
    pub id: String,
    pub title: String,
    pub item_ids: Vec<String>,
}

fn load_store(path: &Path) -> Result<AlbumStore> {
    if !path.exists() {
        return Ok(AlbumStore::default());
    }
    let text = fs::read_to_string(path).map_err(|e| error::io_at(path, e))?;
    Ok(serde_json::from_str(&text)?)
}

fn save_store(path: &Path, store: &AlbumStore) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| error::io_at(parent, e))?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(store)?).map_err(|e| error::io_at(&tmp, e))?;
    fs::rename(&tmp, path).map_err(|e| error::io_at(path, e))?;
    Ok(())
}

fn album_add(path: &Path, album_id: &str, title: &str, item_id: &str) -> Result<()> {
    let mut store = load_store(path)?;
    let album = store
        .albums
        .entry(album_id.to_string())
        .or_insert_with(|| AlbumRecord {
            id: album_id.to_string(),
            title: title.to_string(),
            item_ids: Vec::new(),
        });
    if !album.title.is_empty() && album.title != title {
        album.title = title.to_string();
    }
    if !album.item_ids.iter().any(|i| i == item_id) {
        album.item_ids.push(item_id.to_string());
    }
    save_store(path, &store)
}

fn album_remove(path: &Path, album_id: &str, item_id: &str) -> Result<()> {
    let mut store = load_store(path)?;
    if let Some(album) = store.albums.get_mut(album_id) {
        album.item_ids.retain(|i| i != item_id);
    }
    save_store(path, &store)
}

#[cfg(windows)]
fn windows_volume_serial(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetVolumeInformationW(
            root: *const u16,
            name: *mut u16,
            name_len: u32,
            serial: *mut u32,
            max_comp: *mut u32,
            flags: *mut u32,
            fs: *mut u16,
            fs_len: u32,
        ) -> i32;
        fn GetVolumePathNameW(file: *const u16, out: *mut u16, len: u32) -> i32;
    }
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let mut vol = [0u16; 512];
    let ok = unsafe { GetVolumePathNameW(wide.as_ptr(), vol.as_mut_ptr(), vol.len() as u32) };
    if ok == 0 {
        return None;
    }
    let mut serial: u32 = 0;
    let ok = unsafe {
        GetVolumeInformationW(
            vol.as_ptr(),
            std::ptr::null_mut(),
            0,
            &mut serial,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };
    if ok == 0 {
        None
    } else {
        Some(serial as u64)
    }
}

#[cfg(windows)]
fn windows_free_space(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            path: *const u16,
            avail: *mut u64,
            total: *mut u64,
            free: *mut u64,
        ) -> i32;
    }
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let mut avail = 0u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, std::ptr::null_mut(), std::ptr::null_mut())
    };
    if ok == 0 {
        None
    } else {
        Some(avail)
    }
}
