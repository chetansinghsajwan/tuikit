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
