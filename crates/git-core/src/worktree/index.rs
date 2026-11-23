//! Stage, unstage and discard.

use super::*;

impl Repo {
    /// Stage changes (new, modified and deleted files) for `paths`; a path inside a submodule is
    /// staged in the submodule's own repo.
    pub fn stage(&self, paths: &[PathBuf]) -> Result<(), Error> {
        let (own, groups) = self.by_owner(paths);
        if !own.is_empty() {
            self.git(&["add", "-A"], &own)?;
        }
        groups.iter().try_for_each(|(repo, inner)| repo.stage(inner))
    }

    /// Stage everything.
    pub fn stage_all(&self) -> Result<(), Error> {
        self.git(&["add", "-A"], &[]).map(|_| ())
    }

    /// Unstage `paths` (keeps the work tree as is); a path inside a submodule is unstaged in the
    /// submodule's own repo.
    pub fn unstage(&self, paths: &[PathBuf]) -> Result<(), Error> {
        let (own, groups) = self.by_owner(paths);
        if !own.is_empty() {
            if self.has_head() {
                self.git(&["restore", "--staged"], &own)?;
            } else {
                // No commit yet: unstaging means removing the files from the index.
                self.git(&["rm", "-r", "--cached", "-q"], &own)?;
            }
        }
        groups.iter().try_for_each(|(repo, inner)| repo.unstage(inner))
    }

    /// Unstage everything (keeps the work tree as is).
    pub fn unstage_all(&self) -> Result<(), Error> {
        if self.has_head() {
            self.git(&["reset", "-q"], &[]).map(|_| ())
        } else {
            self.git(&["rm", "-r", "--cached", "-q", "."], &[]).map(|_| ())
        }
    }

    /// Throw away work-tree changes: tracked files are restored from the index,
    /// untracked files are deleted. Not recoverable.
    pub fn discard(&self, rel_path: &Path, untracked: bool) -> Result<(), Error> {
        if let Some((repo, inner)) = self.owner(rel_path) {
            return repo.discard(&inner, untracked);
        }
        if untracked {
            let path = self.root.join(rel_path);
            let meta = path.symlink_metadata().map_err(err)?;
            return if meta.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            }
            .map_err(err);
        }
        self.git(&["restore"], &[rel_path.to_path_buf()]).map(|_| ())
    }
}
