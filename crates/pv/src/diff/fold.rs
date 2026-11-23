use crate::git::PatchKind;
use git_core::FilePatch;

thread_local! {
    /// Files the reader folded or unfolded by hand (a file that folds by itself, a lockfile say, is in here when opened).
    static TOGGLED: std::cell::RefCell<std::collections::HashSet<String>> = std::cell::RefCell::new(Default::default());
    /// Whether whitespace-only changes are drawn as unchanged lines (see `set_ignore_whitespace`).
    static IGNORE_WHITESPACE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Fold the file `path` of the patch, or unfold it.
pub fn toggle_fold(path: &str) {
    TOGGLED.with(|t| {
        let mut t = t.borrow_mut();
        if !t.remove(path) {
            t.insert(path.to_string());
        }
    });
}

/// Forget the hand-made folds (another commit is shown).
pub fn reset_folds() {
    TOGGLED.with(|t| t.borrow_mut().clear());
}

/// Show changes that only differ in whitespace as unchanged lines.
pub fn set_ignore_whitespace(on: bool) {
    IGNORE_WHITESPACE.with(|w| w.set(on));
}

/// Whether whitespace-only changes are currently drawn as unchanged lines.
pub fn ignoring_whitespace() -> bool {
    IGNORE_WHITESPACE.with(|w| w.get())
}

/// Files nobody reads line by line: lockfiles, minified and generated output, and anything that changes this much.
pub(super) const BIG_FILE: usize = 400;

/// Whether a file starts folded: lockfiles, minified or generated output, or a huge change.
pub(super) fn folds_by_itself(f: &FilePatch, changed: usize) -> bool {
    let name = f.path.rsplit('/').next().unwrap_or(&f.path);
    let generated = [
        "Cargo.lock",
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "flake.lock",
        "poetry.lock",
        "go.sum",
        "composer.lock",
        "Gemfile.lock",
    ]
    .contains(&name)
        || name.ends_with(".min.js")
        || name.ends_with(".min.css")
        || name.ends_with(".map");
    generated || changed > BIG_FILE
}

/// Whether the file is drawn as its header only.
pub fn is_folded(f: &FilePatch, changed: usize) -> bool {
    folds_by_itself(f, changed) != TOGGLED.with(|t| t.borrow().contains(&f.path))
}

/// Added and removed lines of a file.
pub fn counts(f: &FilePatch) -> (usize, usize) {
    let (mut a, mut r) = (0, 0);
    for l in f.hunks.iter().flat_map(|h| &h.lines) {
        match l.kind {
            PatchKind::Added => a += 1,
            PatchKind::Removed => r += 1,
            PatchKind::Context => {}
        }
    }
    (a, r)
}
