use super::*;
use std::fs;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("files-ops-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn create_makes_files_and_folders_and_parents_and_refuses_bad_names() {
    let d = dir("create");
    let (f, is_dir) = create(&d, "new.txt").unwrap();
    assert!(f.is_file() && !is_dir);
    let (sub, is_dir) = create(&d, "a/b/").unwrap();
    assert!(sub.is_dir() && is_dir);
    let (nested, _) = create(&d, "x/y/z.rs").unwrap();
    assert!(nested.is_file());
    assert_eq!(create(&d, "new.txt").unwrap_err().kind(), io::ErrorKind::AlreadyExists);
    for bad in ["", "/", "../escape", "/abs", "a/../../b"] {
        assert_eq!(create(&d, bad).unwrap_err().kind(), io::ErrorKind::InvalidInput, "{bad:?}");
    }
}

#[test]
fn rename_moves_within_the_folder_and_never_overwrites() {
    let d = dir("rename");
    fs::write(d.join("a.txt"), "a").unwrap();
    fs::write(d.join("b.txt"), "b").unwrap();
    assert_eq!(rename(&d.join("a.txt"), "c.txt").unwrap(), d.join("c.txt"));
    assert!(d.join("c.txt").is_file() && !d.join("a.txt").exists());
    assert_eq!(rename(&d.join("c.txt"), "b.txt").unwrap_err().kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read_to_string(d.join("b.txt")).unwrap(), "b");
    assert_eq!(rename(&d.join("c.txt"), "sub/dir/d.txt").unwrap(), d.join("sub/dir/d.txt"));
    assert!(rename(&d.join("sub/dir/d.txt"), "../../x").is_err());
    // Renaming to itself is a no-op.
    assert_eq!(rename(&d.join("b.txt"), "b.txt").unwrap(), d.join("b.txt"));
}

#[test]
fn unique_keeps_the_extension_last() {
    let d = dir("unique");
    assert_eq!(unique(d.join("a.txt")), d.join("a.txt"));
    fs::write(d.join("a.txt"), "").unwrap();
    assert_eq!(unique(d.join("a.txt")), d.join("a copy.txt"));
    fs::write(d.join("a copy.txt"), "").unwrap();
    assert_eq!(unique(d.join("a.txt")), d.join("a copy 2.txt"));
    fs::create_dir(d.join("dir")).unwrap();
    assert_eq!(unique(d.join("dir")), d.join("dir copy"));
}

#[test]
fn copy_is_recursive_keeps_symlinks_and_refuses_self_copy() {
    let d = dir("copy");
    fs::create_dir_all(d.join("src/inner")).unwrap();
    fs::write(d.join("src/a.txt"), "a").unwrap();
    fs::write(d.join("src/inner/b.txt"), "b").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("a.txt", d.join("src/link")).unwrap();
    copy(&d.join("src"), &d.join("dst")).unwrap();
    assert_eq!(fs::read_to_string(d.join("dst/inner/b.txt")).unwrap(), "b");
    #[cfg(unix)]
    assert_eq!(fs::read_link(d.join("dst/link")).unwrap(), PathBuf::from("a.txt"));
    assert!(copy(&d.join("src"), &d.join("src/inner/again")).is_err());
}

#[test]
fn move_and_remove_work_on_files_and_folders() {
    let d = dir("move");
    fs::create_dir_all(d.join("f/g")).unwrap();
    fs::write(d.join("f/g/x"), "x").unwrap();
    fs::write(d.join("one"), "1").unwrap();
    move_to(&d.join("one"), &d.join("f/one")).unwrap();
    assert!(d.join("f/one").is_file() && !d.join("one").exists());
    assert!(move_to(&d.join("f"), &d.join("f/g/inside")).is_err());
    remove(&d.join("f")).unwrap();
    assert!(!d.join("f").exists());
}

#[test]
fn trash_removes_the_file_from_its_place() {
    let d = dir("trash");
    let f = d.join("gone.txt");
    fs::write(&f, "x").unwrap();
    // Trash needs a writable trash directory; skip quietly where there is none.
    match trash(std::slice::from_ref(&f)) {
        Ok(()) => assert!(!f.exists()),
        Err(e) => eprintln!("trash unavailable here: {e}"),
    }
}

#[test]
fn bulk_rename_plans_swaps_and_refuses_bad_edits() {
    let d = dir("bulk");
    for n in ["a", "b", "c"] {
        fs::write(d.join(n), n).unwrap();
    }
    let olds = vec![d.join("a"), d.join("b")];
    assert_eq!(bulk_rename_text(&d, &olds), "a\nb\n");
    // A swap is allowed and works through temporary names.
    let moves = plan_bulk_rename(&d, &olds, "b\na\n").unwrap();
    assert_eq!(moves.len(), 2);
    apply_moves(&moves).unwrap();
    assert_eq!(fs::read_to_string(d.join("a")).unwrap(), "b");
    assert_eq!(fs::read_to_string(d.join("b")).unwrap(), "a");
    // Unchanged lines make no moves; a new folder is created on demand.
    assert!(plan_bulk_rename(&d, &olds, "a\nb\n").unwrap().is_empty());
    apply_moves(&plan_bulk_rename(&d, &olds, "a\nsub/x\n").unwrap()).unwrap();
    assert!(d.join("sub/x").is_file());
    // Refusals.
    let olds = vec![d.join("a"), d.join("c")];
    assert!(plan_bulk_rename(&d, &olds, "a\n").is_err()); // line count
    assert!(plan_bulk_rename(&d, &olds, "z\nz\n").is_err()); // duplicate
    assert!(plan_bulk_rename(&d, &olds, "a\n../up\n").is_err()); // escapes
    assert!(plan_bulk_rename(&d, &olds, "sub/x\nc\n").is_err()); // exists, not renamed away
}
