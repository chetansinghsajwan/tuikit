//! Watch a folder for changes (native file events: inotify, FSEvents), so tools show what
//! happened outside them (a commit in another terminal, a file saved elsewhere) without polling.
//!
//! Events are debounced: the callback runs once, shortly after a burst of changes ends.
//! Git's object store and build output are ignored, they change constantly and mean nothing.
//!
//! Only the folders that matter are watched, one by one: git-ignored folders (build output,
//! `.direnv`), symlinks (they lead into the Nix store, tens of thousands of folders) and git's
//! object store are never entered. The folders are found on a background thread, so starting a
//! watch never delays a tool's first frame.

use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ignore::WalkBuilder;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

/// How long the folder must be quiet before the callback runs.
const QUIET: Duration = Duration::from_millis(250);

/// What the event callback tells the worker thread.
enum Msg {
    /// Something that matters changed.
    Changed,
    /// A folder appeared: watch it (and what is inside) too.
    NewDir(PathBuf),
}

/// A running watch; dropping it stops the watch.
pub struct Watch {
    _inner: Arc<Mutex<RecommendedWatcher>>,
    flag: Arc<AtomicBool>,
}

impl Watch {
    /// Watch `root` and call `on_change` (from a background thread) after each burst of changes.
    /// `None` when the system cannot watch at all; folders past its watch limit are skipped.
    pub fn new(root: &Path, on_change: impl Fn() + Send + 'static) -> Option<Watch> {
        let (tx, rx) = mpsc::channel::<Msg>();
        // The callback never takes the watcher's lock: `watch` waits for the event thread, which
        // runs this callback, so locking here could deadlock. It only sends.
        let mut inner = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else { return };
            // Reading a file is not a change (inotify reports every open).
            if matches!(event.kind, EventKind::Access(_)) || !event.paths.iter().any(|p| matters(p))
            {
                return;
            }
            if matches!(event.kind, EventKind::Create(_)) {
                for p in event.paths.iter().filter(|p| p.is_dir() && !p.is_symlink()) {
                    let _ = tx.send(Msg::NewDir(p.clone()));
                }
            }
            let _ = tx.send(Msg::Changed);
        })
        .ok()?;
        // The root itself is watched right away (one call), the rest is found in the background.
        inner.watch(root, RecursiveMode::NonRecursive).ok()?;
        let inner = Arc::new(Mutex::new(inner));
        let flag = Arc::new(AtomicBool::new(false));

        let watcher = Arc::downgrade(&inner);
        let seen = flag.clone();
        let root = root.to_path_buf();
        std::thread::spawn(move || {
            for dir in dirs_to_watch(&root).into_iter().filter(|d| *d != root) {
                let Some(w) = watcher.upgrade() else { return };
                let _ = w
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .watch(&dir, RecursiveMode::NonRecursive);
            }
            // Ends when the watcher (and with it the sender) is dropped.
            while let Ok(msg) = rx.recv() {
                let mut changed = false;
                let mut next = Some(msg);
                while let Some(msg) = next {
                    match msg {
                        Msg::Changed => changed = true,
                        Msg::NewDir(dir) => {
                            for d in dirs_to_watch(&dir) {
                                let Some(w) = watcher.upgrade() else { return };
                                let _ = w
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .watch(&d, RecursiveMode::NonRecursive);
                            }
                        }
                    }
                    next = rx.recv_timeout(QUIET).ok();
                }
                if changed {
                    seen.store(true, Ordering::Relaxed);
                    on_change();
                }
            }
        });
        Some(Watch { _inner: inner, flag })
    }

    /// Whether something changed since the last call (for tools that poll in their loop).
    pub fn take(&self) -> bool {
        self.flag.swap(false, Ordering::Relaxed)
    }
}

/// The folders under `root` (itself included) worth watching: the working tree without git-ignored
/// folders, symlinks and `.git`, plus the few folders of `.git` that hold HEAD, the index and refs.
fn dirs_to_watch(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .follow_links(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build()
        .flatten()
        .filter(|e| e.file_type().is_some_and(|t| t.is_dir()))
        .map(|e| e.into_path())
        .collect();
    let git = root.join(".git");
    if git.is_dir() {
        dirs.push(git.clone());
        // `refs` is small (branches, tags, remotes); `objects`, `logs` and `hooks` are skipped.
        dirs.extend(
            WalkBuilder::new(git.join("refs"))
                .standard_filters(false)
                .follow_links(false)
                .build()
                .flatten()
                .filter(|e| e.file_type().is_some_and(|t| t.is_dir()))
                .map(|e| e.into_path()),
        );
    }
    dirs
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
mod tests;
