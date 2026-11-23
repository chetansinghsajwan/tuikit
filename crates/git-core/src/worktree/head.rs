//! The current branch and whether `HEAD` exists.

use super::*;

impl Repo {
    /// The checked-out branch, or `None` on a detached HEAD.
    pub fn branch(&self) -> Option<String> {
        let name = self.repo.head_name().ok()??;
        Some(name.shorten().to_str_lossy().into_owned())
    }

    /// Whether the repo has a commit yet, since a fresh repo needs other commands to unstage.
    pub(super) fn has_head(&self) -> bool {
        self.repo.head_commit().is_ok()
    }
}
