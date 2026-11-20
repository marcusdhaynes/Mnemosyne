//! Removable drives and phones mounted as a filesystem (Windows-first).
//! Direct MTP via Windows WPD / libmtp is a documented stub — see [`mtp_stub`].

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::file::{source_id_for, Enumeration, SourceKind};
use crate::formats::FormatResolver;
use crate::sources::{walk_enumeration, SourceAdapter};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredDevice {
    pub path: PathBuf,
    pub label: String,
    pub device_kind: String,
    pub drive_kind: String,
    pub layout_hint: String,
}

pub struct RemovableDeviceAdapter {
    root: PathBuf,
    label: String,
    device_kind: String,
    layout_hint: String,
}

impl RemovableDeviceAdapter {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let root = path.as_ref().to_path_buf();
        let hint = classify_layout(&root);
        let label = volume_label(&root).unwrap_or_else(|| hint.label.clone());
        Self {
            root,
            label,
            device_kind: hint.device_kind,
            layout_hint: hint.layout_hint,
        }
    }
}

impl SourceAdapter for RemovableDeviceAdapter {
    fn kind(&self) -> SourceKind {
        SourceKind::Device
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
            source_id_for(SourceKind::Device, &root),
            SourceKind::Device,
            None,
            self.label.clone(),
            Some(self.device_kind.clone()),
            Some(self.layout_hint.clone()),
            resolver,
        )
    }
}

#[derive(Clone)]
struct Layout {
    device_kind: String,
    layout_hint: String,
    label: String,
}

fn child_exists(root: &Path, name: &str) -> bool {
    root.join(name).is_dir() || root.join(name).is_file()
}

fn dir_names(path: &Path) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(rd) = std::fs::read_dir(path) {
        for e in rd.flatten() {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                names.push(e.file_name().to_string_lossy().to_string());
            }
        }
    }
    names
}

/// True if this folder looks like a camera, phone, or SD card (DCIM-style layouts).
pub fn looks_like_device(root: &Path) -> bool {
    let dcim = root.join("DCIM");
    if dcim.is_dir() {
        return true;
    }
    // Some Android volumes expose DCIM next to Pictures at the mount root,
    // or a user copied the DCIM tree itself.
    let names = dir_names(root);
    if names.iter().any(|n| n.eq_ignore_ascii_case("DCIM")) {
        return true;
    }
    // A DCIM roll folder itself (100APPLE, 100CANON, Camera).
    let upper = root
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_uppercase())
        .unwrap_or_default();
    if upper.ends_with("APPLE") || upper == "CAMERA" || upper.ends_with("CANON") || upper == "100MSDCF"
    {
        return true;
    }
    child_exists(root, "PRIVATE") || child_exists(root, "MP_ROOT")
}

fn classify_layout(root: &Path) -> Layout {
    let dcim = if root.join("DCIM").is_dir() {
        root.join("DCIM")
    } else {
        root.to_path_buf()
    };
    let rolls = dir_names(&dcim);
    let apple = rolls.iter().any(|n| n.to_ascii_uppercase().contains("APPLE"));
    let android_camera = rolls.iter().any(|n| n.eq_ignore_ascii_case("Camera"))
        || root.join("Pictures").is_dir()
        || rolls.iter().any(|n| n.to_ascii_uppercase().contains("ANDRO"));
    let camera = rolls.iter().any(|n| {
        let u = n.to_ascii_uppercase();
        u.contains("CANON")
            || u.contains("NIKON")
            || u.contains("MSDCF")
            || u.contains("OLYMP")
            || u.contains("FUJI")
            || u.contains("PANASONIC")
    });
    if apple {
        return Layout {
            device_kind: "iphone".into(),
            layout_hint: "dcim_apple".into(),
            label: "iPhone (DCIM)".into(),
        };
    }
    if android_camera && !camera {
        return Layout {
            device_kind: "android".into(),
            layout_hint: "dcim_android".into(),
            label: "Android (DCIM)".into(),
        };
    }
    if camera {
        return Layout {
            device_kind: "camera".into(),
            layout_hint: "dcim_camera".into(),
            label: "Camera / SD card".into(),
        };
    }
    if looks_like_device(root) {
        return Layout {
            device_kind: "removable".into(),
            layout_hint: "dcim".into(),
            label: "Removable media".into(),
        };
    }
    Layout {
        device_kind: "drive".into(),
        layout_hint: "volume".into(),
        label: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Drive".into()),
    }
}

fn volume_label(_root: &Path) -> Option<String> {
    #[cfg(windows)]
    {
        return windows_volume_label(_root);
    }
    #[allow(unreachable_code)]
    None
}

/// Discover removable / phone-as-drive volumes. Extra roots are always considered
/// (used on Linux tests and for user-picked mount points).
pub fn discover_devices(extra_roots: &[PathBuf]) -> Result<Vec<DiscoveredDevice>> {
    let mut candidates = extra_roots.to_vec();
    candidates.extend(platform_roots());
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for root in candidates {
        let key = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
        if !key.is_dir() || !seen.insert(key.clone()) {
            continue;
        }
        let drive_kind = platform_drive_kind(&key);
        let is_interesting = looks_like_device(&key)
            || drive_kind == "removable"
            || drive_kind == "cdrom"
            || !extra_roots.is_empty() && extra_roots.iter().any(|p| {
                std::fs::canonicalize(p).ok().as_ref() == Some(&key) || p == &root
            });
        if !is_interesting {
            continue;
        }
        // If we were given a volume root (E:\), prefer it; if it contains DCIM, keep the volume
        // as the source root so Android Pictures/ and DCIM/ are both included.
        let hint = classify_layout(&key);
        let label = volume_label(&key).unwrap_or(hint.label);
        out.push(DiscoveredDevice {
            path: key,
            label,
            device_kind: hint.device_kind,
            drive_kind,
            layout_hint: hint.layout_hint,
        });
    }
    Ok(out)
}

fn platform_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    #[cfg(windows)]
    {
        roots.extend(windows_logical_drives());
        // iCloud for Windows synced folder, if present (not an API; just a folder route).
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let photos = PathBuf::from(profile).join("iCloudPhotos").join("Photos");
            if photos.is_dir() {
                roots.push(photos);
            }
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let media = PathBuf::from("/media").join(PathBuf::from(&home).file_name().unwrap_or_default());
            if media.is_dir() {
                if let Ok(rd) = std::fs::read_dir(&media) {
                    roots.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
                }
            }
        }
        let run = PathBuf::from("/run/media");
        if run.is_dir() {
            if let Ok(users) = std::fs::read_dir(&run) {
                for user in users.flatten() {
                    if let Ok(vols) = std::fs::read_dir(user.path()) {
                        roots.extend(vols.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
                    }
                }
            }
        }
        let mnt = PathBuf::from("/mnt");
        if mnt.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&mnt) {
                roots.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
            }
        }
    }
    roots
}

fn platform_drive_kind(_path: &Path) -> String {
    #[cfg(windows)]
    {
        return windows_drive_kind(_path);
    }
    #[allow(unreachable_code)]
    "mounted".into()
}

/// Direct MTP (a phone that did not appear as a drive letter) is not implemented.
/// On Windows the route is WPD; on other platforms, libmtp. Prefer "File transfer"
/// / PTP so the phone mounts as a drive, then use [`discover_devices`].
pub fn mtp_stub() -> crate::error::Error {
    crate::error::Error::Stub {
        feature: "MTP / Windows WPD",
        route: "Phones should be imported over USB as a drive letter (File transfer or PTP) so DCIM is a normal folder. Direct MTP via Windows Portable Devices (WPD) or libmtp is not implemented in this build and makes no device calls.",
    }
}

#[cfg(windows)]
fn windows_logical_drives() -> Vec<PathBuf> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetLogicalDrives() -> u32;
    }
    let mask = unsafe { GetLogicalDrives() };
    let mut out = Vec::new();
    for i in 0..26u8 {
        if mask & (1 << i) != 0 {
            let letter = (b'A' + i) as char;
            out.push(PathBuf::from(format!("{letter}:\\")));
        }
    }
    out
}

#[cfg(windows)]
fn windows_drive_kind(path: &Path) -> String {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDriveTypeW(root: *const u16) -> u32;
    }
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if !wide.ends_with(&[0]) {
        wide.push(0);
    }
    match unsafe { GetDriveTypeW(wide.as_ptr()) } {
        2 => "removable".into(),
        3 => "fixed".into(),
        4 => "remote".into(),
        5 => "cdrom".into(),
        6 => "ramdisk".into(),
        _ => "unknown".into(),
    }
}

#[cfg(windows)]
fn windows_volume_label(path: &Path) -> Option<String> {
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
    }
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if !wide.ends_with(&[0]) {
        wide.push(0);
    }
    let mut name = [0u16; 261];
    let ok = unsafe {
        GetVolumeInformationW(
            wide.as_ptr(),
            name.as_mut_ptr(),
            name.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };
    if ok == 0 {
        return None;
    }
    let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    let s = String::from_utf16_lossy(&name[..len]);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
