//! Which repository owns a path (a submodule or this one).

use super::*;

impl Repo {
    /// The repo that owns `rel_path` when it lies inside a submodule (nested ones followed), with the
    /// path inside that repo. `None` when this repo owns it, which includes the submodule's own
    /// path: that is the recorded commit, a change of this repo.
    pub fn owner(&self, rel_path: &Path) -> Option<(Repo, PathBuf)> {
        let subs = self.repo.submodules().ok()??;
        for sub in subs {
            let Ok(path) = sub.path() else { continue };
            let sub_rela = PathBuf::from(path.to_str_lossy().into_owned());
            let Ok(inner) = rel_path.strip_prefix(&sub_rela) else { continue };
            if inner.as_os_str().is_empty() {
                continue;
            }
            // Not cloned: discovery would walk up to this repo, so check the root is the submodule's.
            let dir = self.root.join(&sub_rela);
            let Some(repo) =
                Repo::discover(&dir).filter(|r| Some(&r.root) == dir.canonicalize().ok().as_ref())
            else {
                continue;
            };
            return Some(match repo.owner(inner) {
                Some(deeper) => deeper,
                None => (repo, inner.to_path_buf()),
            });
        }
        None
    }

    /// Split `paths` into the ones this repo owns and, per submodule repo, the ones inside it.
    pub(super) fn by_owner(&self, paths: &[PathBuf]) -> (Vec<PathBuf>, Vec<(Repo, Vec<PathBuf>)>) {
        let mut own = Vec::new();
        let mut groups: Vec<(Repo, Vec<PathBuf>)> = Vec::new();
        for path in paths {
            match self.owner(path) {
                None => own.push(path.clone()),
                Some((repo, inner)) => match groups.iter_mut().find(|(r, _)| r.root == repo.root) {
                    Some((_, list)) => list.push(inner),
                    None => groups.push((repo, vec![inner])),
                },
            }
        }
        (own, groups)
    }
}
