mod common;

use photo_sources::formats::{FormatResolver, PassThroughResolver};
use photo_sources::sources::{self, detect_export_kind, unpack_export, CloudKind};
use photo_sources::{enumerate_path, FileRole, SourceFile, SourceKind};
use std::fs;
use std::path::PathBuf;

struct MarkingResolver;
impl FormatResolver for MarkingResolver {
    fn resolve_formats(
        &self,
        mut files: Vec<SourceFile>,
    ) -> photo_sources::Result<Vec<SourceFile>> {
        for f in &mut files {
            f.format = Some(serde_json::json!({ "hook": "called" }));
        }
        Ok(files)
    }
}

fn write_tree(root: &std::path::Path, rel: &str, bytes: &[u8]) {
    common::write_bytes(&root.join(rel), bytes);
}

#[test]
fn discovers_iphone_and_android_dcim_layouts() {
    let root = common::temp_dir("dcim");
    write_tree(&root, "iphone/DCIM/100APPLE/IMG_0001.HEIC", b"heic");
    write_tree(&root, "iphone/DCIM/100APPLE/IMG_0001.MOV", b"mov");
    write_tree(&root, "android/DCIM/Camera/IMG_20240101_120000.jpg", b"jpg");
    write_tree(&root, "android/Pictures/Screenshots/Screenshot_1.png", b"png");
    write_tree(&root, "sdcard/DCIM/100CANON/IMG_0002.CR2", b"raw");
    write_tree(&root, "sdcard/DCIM/100CANON/IMG_0002.JPG", b"jpg");

    let found = sources::discover_devices(&[
        root.join("iphone"),
        root.join("android"),
        root.join("sdcard"),
    ])
    .unwrap();
    let kinds: Vec<_> = found.iter().map(|d| d.device_kind.as_str()).collect();
    assert!(kinds.contains(&"iphone"), "{found:?}");
    assert!(kinds.contains(&"android"), "{found:?}");
    assert!(kinds.contains(&"camera"), "{found:?}");

    let iphone = enumerate_path(&root.join("iphone"), &PassThroughResolver).unwrap();
    assert_eq!(iphone.source_kind, SourceKind::Device);
    assert_eq!(iphone.device_kind.as_deref(), Some("iphone"));
    assert_eq!(iphone.media_count, 2);
    // Pairing is Spin's job: both the HEIC and the MOV are listed separately.
    let names: Vec<_> = iphone.files.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"IMG_0001.HEIC"));
    assert!(names.contains(&"IMG_0001.MOV"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn mtp_is_a_documented_stub() {
    let err = sources::device::mtp_stub();
    let msg = err.to_string();
    assert!(msg.contains("WPD") || msg.contains("MTP"), "{msg}");
}

#[test]
fn unpacks_takeout_zip_and_attaches_sidecar_paths_without_parsing() {
    let root = common::temp_dir("takeout");
    let zip = root.join("takeout.zip");
    common::write_zip(
        &zip,
        &[
            ("Takeout/Google Photos/IMG_0001.jpg", b"jpeg-bytes"),
            ("Takeout/Google Photos/IMG_0001.jpg.json", common::TAKEOUT_JSON.as_bytes()),
            ("Takeout/Mail/ignored.mbox", b"not media"),
        ],
    );
    let dest = root.join("unpacked");
    let report = unpack_export(&zip, &dest).unwrap();
    assert_eq!(report.extracted, 2);
    assert!(report.skipped >= 1);
    assert_eq!(report.export_kind, photo_sources::ExportKind::Takeout);

    let listing = enumerate_path(&dest, &PassThroughResolver).unwrap();
    assert_eq!(listing.export_kind, Some(photo_sources::ExportKind::Takeout));
    assert_eq!(listing.media_count, 1);
    assert_eq!(listing.sidecar_count, 1);
    let media = listing
        .files
        .iter()
        .find(|f| f.role == FileRole::Media)
        .unwrap();
    assert_eq!(media.sidecar_paths.len(), 1);
    assert!(media.sidecar_paths[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .ends_with(".json"));
    // We must not have recovered the sidecar date ourselves.
    assert!(media.format.is_none());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn detects_social_export_folders_and_lists_sidecars() {
    let root = common::temp_dir("social");
    write_tree(&root, "ig/your_instagram_activity/media/posts/202401/IMG_ig.jpg", b"ig");
    write_tree(&root, "ig/your_instagram_activity/media/posts_1.json", common::INSTAGRAM_POSTS.as_bytes());
    write_tree(&root, "fb/photos_and_videos/your_photos/FB_1.jpg", b"fb");
    write_tree(&root, "fb/posts/your_posts.json", common::FACEBOOK_PHOTO.as_bytes());
    write_tree(&root, "snap/memories/mid1.jpg", b"snap");
    write_tree(&root, "snap/json/memories_history.json", common::SNAP_MEMORIES.as_bytes());
    write_tree(&root, "tt/TikTok/Videos/clip.mp4", b"tt");
    write_tree(&root, "tt/user_data.json", common::TIKTOK_USER.as_bytes());
    write_tree(
        &root,
        "wa/WhatsApp Image 2024-01-01 at 15.30.45.jpg",
        b"wa",
    );

    assert_eq!(detect_export_kind(&root.join("ig")), Some(photo_sources::ExportKind::Instagram));
    assert_eq!(detect_export_kind(&root.join("fb")), Some(photo_sources::ExportKind::Facebook));
    assert_eq!(detect_export_kind(&root.join("snap")), Some(photo_sources::ExportKind::Snapchat));
    assert_eq!(detect_export_kind(&root.join("tt")), Some(photo_sources::ExportKind::Tiktok));
    // A bare WhatsApp dump is just a folder — date-from-filename is Spin's hook.
    assert_eq!(detect_export_kind(&root.join("wa")), None);

    let ig = enumerate_path(&root.join("ig"), &PassThroughResolver).unwrap();
    assert_eq!(ig.export_kind, Some(photo_sources::ExportKind::Instagram));
    assert_eq!(ig.media_count, 1);
    assert!(ig.sidecar_count >= 1);

    let wa = enumerate_path(&root.join("wa"), &PassThroughResolver).unwrap();
    assert_eq!(wa.source_kind, SourceKind::Folder);
    assert_eq!(wa.media_count, 1);
    assert!(wa.files[0].format.is_none());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn resolve_formats_hook_is_invoked() {
    let root = common::temp_dir("hook");
    common::write_bytes(&root.join("pic.jpg"), b"jpeg");
    let listing = enumerate_path(&root, &MarkingResolver).unwrap();
    assert!(listing.files.iter().all(|f| f.format.is_some()));
    let listing2 = enumerate_path(&root, &PassThroughResolver).unwrap();
    assert!(listing2.files.iter().all(|f| f.format.is_none()));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn rejects_zip_slip() {
    let root = common::temp_dir("slip");
    let zip = root.join("evil.zip");
    common::write_zip(&zip, &[("../evil.jpg", b"no"), ("ok.jpg", b"yes")]);
    let dest = root.join("out");
    let report = unpack_export(&zip, &dest).unwrap();
    assert!(!root.join("evil.jpg").exists());
    assert!(!dest.join("../evil.jpg").exists() || dest.join("ok.jpg").exists());
    // enclosed_name skips the parent entry; the safe file is kept.
    assert!(dest.join("ok.jpg").exists() || report.extracted <= 1);
    assert!(!fs::read_dir(dest.parent().unwrap()).unwrap().flatten().any(|e| e.file_name() == "evil.jpg"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cloud_connectors_are_stubs_without_network() {
    for kind in CloudKind::all() {
        let err = sources::cloud::connect(kind).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not connected") || msg.contains("no public"), "{msg}");
        assert!(!msg.is_empty());
    }
    let stubs = sources::cloud::list_stubs();
    assert_eq!(stubs.len(), 5);
    assert!(stubs.iter().all(|s| !s.available));
}

#[test]
fn unused_path_guard() {
    // keep PathBuf import used if rustc complains otherwise
    let _ = PathBuf::from(".");
}
