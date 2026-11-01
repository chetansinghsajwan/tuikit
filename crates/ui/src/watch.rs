//! Watch a folder for changes (native file events: inotify, FSEvents), so tools show what
//! happened outside them (a commit in another terminal, a file saved elsewhere) without polling.
//!
//! Events are debounced: the callback runs once, shortly after a burst of changes ends.
//! Git's object store and build output are ignored, they change constantly and mean nothing.

use std::path::{Component, Path};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

/// How long the folder must be quiet before the callback runs.
const QUIET: Duration = Duration::from_millis(250);

/// A running watch; dropping it stops the watch.
pub struct Watch {
    _inner: RecommendedWatcher,
    flag: Arc<AtomicBool>,
}

impl Watch {
    /// Watch `root` recursively and call `on_change` (from a background thread) after each burst
    /// of changes. `None` when the system cannot watch it (no support, or its watch limit is used up).
    pub fn new(root: &Path, on_change: impl Fn() + Send + 'static) -> Option<Watch> {
        let (tx, rx) = mpsc::channel::<()>();
        let mut inner = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if let Ok(event) = event {
                if event.paths.iter().any(|p| matters(p)) {
                    let _ = tx.send(());
                }
            }
        })
        .ok()?;
        inner.watch(root, RecursiveMode::Recursive).ok()?;
        let flag = Arc::new(AtomicBool::new(false));
        let seen = flag.clone();
        std::thread::spawn(move || {
            // Ends when the watcher (and with it the sender) is dropped.
            while rx.recv().is_ok() {
                while rx.recv_timeout(QUIET).is_ok() {}
                seen.store(true, Ordering::Relaxed);
                on_change();
            }
        });
        Some(Watch { _inner: inner, flag })
    }

    /// Whether something changed since the last call (for tools that poll in their loop).
    pub fn take(&self) -> bool {
        self.flag.swap(false, Ordering::Relaxed)
    }
}

/// Whether a change at `path` is worth showing: not inside git's object store, locks or build output.
fn matters(path: &Path) -> bool {
    let mut prev_git = false;
    for c in path.components() {
        let Component::Normal(name) = c else { continue };
        let name = name.to_string_lossy();
        if prev_git && matches!(name.as_ref(), "objects" | "logs" | "hooks" | "lfs") {
            return false;
        }
        if matches!(name.as_ref(), "target" | "node_modules") || name.ends_with(".lock") {
            return false;
        }
        prev_git = name == ".git";
    }
    true
}

#[cfg(test)]
mod tests {
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
}
