//! The diff of one file, staged or not.

use super::*;

impl Repo {
    /// The bytes of `rel_path` as staged in the index, or `None` if it is not staged.
    pub(super) fn index_blob(&self, rel_path: &Path) -> Option<Vec<u8>> {
        let index = self.repo.index_or_empty().ok()?;
        let entry = index.entry_by_path(rel_path.to_str()?.into())?;
        Some(self.repo.find_blob(entry.id).ok()?.detach().data)
    }

    /// The diff of one file (path relative to the work tree) on `side`; inside a submodule it is
    /// the submodule's own diff.
    pub fn diff_file(&self, rel_path: &Path, side: Side) -> Result<FilePatch, Error> {
        if let Some((repo, inner)) = self.owner(rel_path) {
            return repo.diff_file(&inner, side);
        }
        let (before, after, kind) = match side {
            Side::Staged => {
                let before = head_blob(&self.repo, rel_path);
                let after = self.index_blob(rel_path);
                let kind = match (&before, &after) {
                    (None, _) => Kind::Added,
                    (_, None) => Kind::Deleted,
                    _ => Kind::Modified,
                };
                (before.unwrap_or_default(), after.unwrap_or_default(), kind)
            }
            Side::Unstaged => {
                let before = self.index_blob(rel_path);
                let after = std::fs::read(self.root.join(rel_path)).ok();
                let kind = match (&before, &after) {
                    (None, _) => Kind::Untracked,
                    (_, None) => Kind::Deleted,
                    _ => Kind::Modified,
                };
                (before.unwrap_or_default(), after.unwrap_or_default(), kind)
            }
        };
        let binary = before.contains(&0) || after.contains(&0);
        Ok(FilePatch {
            path: rel_path.to_string_lossy().into_owned(),
            old_path: None,
            modes: None,
            kind,
            binary,
            hunks: if binary { Vec::new() } else { hunks(&before, &after) },
            before: crate::history::full_text(&before),
            after: crate::history::full_text(&after),
        })
    }
}
