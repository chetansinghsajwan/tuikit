//! Stashes.

use super::*;

impl Repo {
    /// Put the work tree and index changes on the stash (`message` may be empty).
    pub fn stash_push(&self, message: &str) -> Result<(), Error> {
        if message.trim().is_empty() {
            self.git(&["stash", "push"], &[])
        } else {
            self.git(&["stash", "push", "-m", message], &[])
        }
        .map(|_| ())
    }

    /// Apply stash entry `index` and keep it.
    pub fn stash_apply(&self, index: usize) -> Result<(), Error> {
        self.git(&["stash", "apply", &format!("stash@{{{index}}}")], &[]).map(|_| ())
    }

    /// Apply stash entry `index` and drop it.
    pub fn stash_pop(&self, index: usize) -> Result<(), Error> {
        self.git(&["stash", "pop", &format!("stash@{{{index}}}")], &[]).map(|_| ())
    }

    /// Drop stash entry `index`.
    pub fn stash_drop(&self, index: usize) -> Result<(), Error> {
        self.git(&["stash", "drop", &format!("stash@{{{index}}}")], &[]).map(|_| ())
    }
}
