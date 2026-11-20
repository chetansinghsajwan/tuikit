//! Tests for the folder watch.

use super::*;
use std::fs;

#[test]
fn git_internals_and_build_output_do_not_matter() {
    assert!(!matters(Path::new("/r/.git/objects/ab/cd")));
    assert!(!matters(Path::new("/r/.git/index.lock")));
    assert!(!matters(Path::new("/r/target/debug/x")));
    assert!(matters(Path::new("/r/.git/HEAD")));
    assert!(matters(Path::new("/r/.git/index")));
    assert!(matters(Path::new("/r/src/main.rs")));
}

#[test]
fn a_change_in_the_folder_calls_back_once_after_the_burst() {
    let dir = std::env::temp_dir().join(format!("ui-watch-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let c = count.clone();
    let Some(watch) = Watch::new(&dir, move || {
        c.fetch_add(1, Ordering::Relaxed);
    }) else {
        return; // the sandbox cannot watch: nothing to test
    };
    for i in 0..5 {
        fs::write(dir.join(format!("f{i}")), "x").unwrap();
    }
    std::thread::sleep(Duration::from_millis(1200));
    assert_eq!(count.load(Ordering::Relaxed), 1);
    assert!(watch.take() && !watch.take());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ignored_folders_symlinks_and_git_internals_are_not_watched() {
    let dir = std::env::temp_dir().join(format!("ui-watch-dirs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for sub in ["src/deep", "build/out", "elsewhere/inner", ".git/objects/ab", ".git/refs/heads"] {
        fs::create_dir_all(dir.join(sub)).unwrap();
    }
    fs::write(dir.join(".gitignore"), "build/\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.join("elsewhere"), dir.join("link")).unwrap();

    let watched = dirs_to_watch(&dir);
    let has = |rel: &str| watched.contains(&dir.join(rel));
    assert!(has("") && has("src/deep") && has(".git") && has(".git/refs/heads"));
    assert!(!has("build") && !has("build/out") && !has(".git/objects") && !has(".git/objects/ab"));
    #[cfg(unix)]
    assert!(!has("link") && !has("link/inner"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_created_later_is_watched() {
    let dir = std::env::temp_dir().join(format!("ui-watch-new-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let Some(watch) = Watch::new(&dir, || {}) else { return };
    std::thread::sleep(Duration::from_millis(300));
    fs::create_dir(dir.join("late")).unwrap();
    std::thread::sleep(Duration::from_millis(800));
    assert!(watch.take());
    fs::write(dir.join("late/f"), "x").unwrap();
    std::thread::sleep(Duration::from_millis(800));
    assert!(watch.take());
    let _ = fs::remove_dir_all(&dir);
}
