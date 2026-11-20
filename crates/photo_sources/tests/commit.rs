mod common;

use photo_sources::commit::{self, CommitMode, CommitPlan, PlanOp};
use photo_sources::Error;
use std::fs;

fn plan(mode: CommitMode, confirmed: bool, ops: Vec<PlanOp>, store: Option<std::path::PathBuf>) -> CommitPlan {
    CommitPlan {
        id: "c_test".into(),
        mode,
        confirmed,
        ops,
        album_store: store,
    }
}

#[test]
fn album_only_is_the_default_and_does_not_move_files() {
    assert_eq!(CommitMode::default(), CommitMode::AlbumOnly);
    assert_eq!(photo_sources::DEFAULT_COMMIT_MODE, CommitMode::AlbumOnly);

    let root = common::temp_dir("album");
    let src = root.join("IMG.jpg");
    fs::write(&src, b"photo").unwrap();
    let store_path = root.join("albums.json");
    let journal = root.join("journal.json");
    let p = plan(
        CommitMode::AlbumOnly,
        false,
        vec![PlanOp::AlbumAdd {
            item_id: "e_1".into(),
            album_id: "a_1".into(),
            album_title: "Trip".into(),
        }],
        Some(store_path.clone()),
    );
    let dry = commit::dry_run(&p).unwrap();
    assert!(dry.ok, "{dry:?}");
    commit::apply(&p, &journal).unwrap();
    assert!(src.exists(), "original must stay in place");
    let store: commit::AlbumStore = serde_json::from_str(&fs::read_to_string(&store_path).unwrap()).unwrap();
    assert_eq!(store.albums["a_1"].item_ids, vec!["e_1".to_string()]);
    commit::undo(&journal).unwrap();
    let store: commit::AlbumStore = serde_json::from_str(&fs::read_to_string(&store_path).unwrap()).unwrap();
    assert!(store.albums["a_1"].item_ids.is_empty());
    assert!(src.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn move_without_confirmation_is_refused() {
    let root = common::temp_dir("noconfirm");
    let src = root.join("a.jpg");
    let dest = root.join("b.jpg");
    fs::write(&src, b"x").unwrap();
    let p = plan(
        CommitMode::Move,
        false,
        vec![PlanOp::Move {
            from: src.clone(),
            to: dest.clone(),
        }],
        None,
    );
    let dry = commit::dry_run(&p).unwrap();
    assert!(!dry.ok);
    assert!(dry.confirmation_needed.is_some());
    let err = commit::apply(&p, &root.join("j.json")).unwrap_err();
    assert!(matches!(err, Error::ConfirmationRequired(_)), "{err}");
    assert!(src.exists());
    assert!(!dest.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn dry_run_detects_missing_source_and_dest_conflicts() {
    let root = common::temp_dir("conflicts");
    let missing = root.join("nope.jpg");
    let src = root.join("a.jpg");
    let dest = root.join("dest.jpg");
    fs::write(&src, b"aaa").unwrap();
    fs::write(&dest, b"other").unwrap();
    let p = plan(
        CommitMode::Copy,
        true,
        vec![
            PlanOp::Copy {
                from: missing,
                to: root.join("out1.jpg"),
            },
            PlanOp::Copy {
                from: src,
                to: dest,
            },
        ],
        None,
    );
    let dry = commit::dry_run(&p).unwrap();
    assert!(!dry.ok);
    assert!(dry.conflicts.len() >= 2, "{:?}", dry.conflicts);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn copy_then_undo_leaves_the_original_and_removes_the_copy() {
    let root = common::temp_dir("copy");
    let src = root.join("orig.jpg");
    let dest = root.join("album/orig.jpg");
    fs::write(&src, b"unique-bytes-42").unwrap();
    let p = plan(
        CommitMode::Copy,
        true,
        vec![
            PlanOp::Mkdir {
                path: root.join("album"),
            },
            PlanOp::Copy {
                from: src.clone(),
                to: dest.clone(),
            },
        ],
        None,
    );
    commit::apply(&p, &root.join("j.json")).unwrap();
    assert!(src.exists());
    assert_eq!(fs::read(&dest).unwrap(), b"unique-bytes-42");
    commit::undo(&root.join("j.json")).unwrap();
    assert!(src.exists(), "undo of copy must not delete the original");
    assert!(!dest.exists(), "undo of copy removes the file we created");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn confirmed_same_volume_move_and_undo() {
    let root = common::temp_dir("move");
    let src = root.join("inbox/pic.jpg");
    let dest = root.join("kept/pic.jpg");
    fs::create_dir_all(src.parent().unwrap()).unwrap();
    fs::write(&src, b"moved-bytes").unwrap();
    let p = plan(
        CommitMode::Move,
        true,
        vec![PlanOp::Move {
            from: src.clone(),
            to: dest.clone(),
        }],
        None,
    );
    commit::apply(&p, &root.join("j.json")).unwrap();
    assert!(!src.exists());
    assert_eq!(fs::read(&dest).unwrap(), b"moved-bytes");
    commit::undo(&root.join("j.json")).unwrap();
    assert!(src.exists());
    assert_eq!(fs::read(&src).unwrap(), b"moved-bytes");
    assert!(!dest.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn rename_stays_in_the_same_folder() {
    let root = common::temp_dir("rename");
    let src = root.join("old.jpg");
    let dest = root.join("new.jpg");
    fs::write(&src, b"r").unwrap();
    let p = plan(
        CommitMode::RenameOnly,
        true,
        vec![PlanOp::Rename {
            from: src.clone(),
            to: dest.clone(),
        }],
        None,
    );
    commit::apply(&p, &root.join("j.json")).unwrap();
    assert!(!src.exists());
    assert!(dest.exists());
    commit::undo(&root.join("j.json")).unwrap();
    assert!(src.exists());
    assert!(!dest.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn album_only_rejects_file_moves() {
    let root = common::temp_dir("reject");
    let src = root.join("a.jpg");
    fs::write(&src, b"a").unwrap();
    let p = plan(
        CommitMode::AlbumOnly,
        false,
        vec![PlanOp::Move {
            from: src.clone(),
            to: root.join("b.jpg"),
        }],
        None,
    );
    let dry = commit::dry_run(&p).unwrap();
    assert!(!dry.ok);
    assert!(dry.conflicts.iter().any(|c| c.message.contains("album_only")));
    let _ = fs::remove_dir_all(&root);
}
