//! `git-core`: the one place git logic lives. Repo discovery, per-file status
//! and per-line changes, on `gix`. Used by `pv`, `pick`, the files panel and
//! `git-panel` so none of them talk to git themselves.

mod graph;
mod history;
mod lines;
mod status;
mod submodule;
mod words;
mod worktree;

pub use graph::{GraphRow, graph, simplify, topo_order};
pub use history::{
    CommitInfo, FileChange, FilePatch, Hunk, PatchKind, PatchLine, RefInfo, RefKind, StashInfo,
};
pub use lines::{LineChange, LineKind};
pub use status::{FileStatus, Kind, Status};
pub use submodule::{SubmoduleInfo, SubmoduleState};
/// The hunks (3 lines of context) of the difference between two versions of a file.
pub fn hunks_of(before: &[u8], after: &[u8]) -> Vec<Hunk> {
    history::hunks(before, after)
}

pub use words::{Intraline, intraline};
pub use worktree::{Operation, ResetMode, Side};

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

fn err(e: impl std::fmt::Display) -> Error {
    Error(e.to_string())
}

/// A discovered repository with a work tree.
pub struct Repo {
    repo: gix::Repository,
    root: PathBuf,
}

impl Repo {
    /// Find the repository containing `path`. `None` if there is none, or it
    /// is bare.
    pub fn discover(path: &Path) -> Option<Repo> {
        let dir = if path.is_dir() { path } else { path.parent()? };
        let repo = gix::discover(dir).ok()?;
        let root = repo.workdir()?.canonicalize().ok()?;
        Some(Repo { repo, root })
    }

    /// The work tree root (canonical).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Repo-relative path for `path`, if it lies inside the work tree.
    pub fn relative(&self, path: &Path) -> Option<PathBuf> {
        let abs = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().ok()?.join(path)
        };
        // Canonicalize the parent so a missing (deleted) file still resolves.
        let canon = match abs.canonicalize() {
            Ok(p) => p,
            Err(_) => abs.parent()?.canonicalize().ok()?.join(abs.file_name()?),
        };
        canon.strip_prefix(&self.root).ok().map(Path::to_path_buf)
    }

    /// Status of every changed path in the repo. Compute once and cache.
    pub fn status(&self) -> Result<Status, Error> {
        status::compute(&self.repo)
    }

    /// Branches, remote branches and tags: the checked-out branch first, then
    /// local branches, remotes and tags, newest first.
    pub fn refs(&self) -> Result<Vec<RefInfo>, Error> {
        history::refs(&self.repo)
    }

    /// Up to `limit` commits reachable from any branch, tag or `HEAD`, newest first, children before parents.
    pub fn commits_all(&self, limit: usize) -> Result<Vec<CommitInfo>, Error> {
        history::commits_all(&self.repo, limit)
    }

    /// Up to `limit` commits reachable from `rev` (a branch, tag or hash), newest first.
    pub fn commits_from(&self, rev: &str, limit: usize) -> Result<Vec<CommitInfo>, Error> {
        history::commits_from(&self.repo, rev, limit)
    }

    /// Up to `limit` commits reachable from `HEAD`, newest first.
    pub fn commits(&self, limit: usize) -> Result<Vec<CommitInfo>, Error> {
        history::commits(&self.repo, limit)
    }

    /// Stash entries, newest (`stash@{0}`) first.
    pub fn stashes(&self) -> Result<Vec<StashInfo>, Error> {
        history::stashes(&self.repo)
    }

    /// One commit by hash, ref name or any rev-spec.
    pub fn commit(&self, rev: &str) -> Result<CommitInfo, Error> {
        history::commit(&self.repo, rev)
    }

    /// Files changed by `rev` against its first parent, with line counts.
    pub fn changes(&self, rev: &str) -> Result<Vec<FileChange>, Error> {
        history::changes(&self.repo, None, rev)
    }

    /// Files that differ between `base` and `rev` (two commits, any distance apart), with line counts.
    pub fn changes_between(&self, base: &str, rev: &str) -> Result<Vec<FileChange>, Error> {
        history::changes(&self.repo, Some(base), rev)
    }

    /// The patch that turns `base` into `rev`.
    pub fn patch_between(&self, base: &str, rev: &str) -> Result<Vec<FilePatch>, Error> {
        history::patch(&self.repo, Some(base), rev)
    }

    /// The patch of `rev` against its first parent: per file, hunks with 3
    /// lines of context. Binary files have no hunks.
    pub fn patch(&self, rev: &str) -> Result<Vec<FilePatch>, Error> {
        history::patch(&self.repo, None, rev)
    }

    /// Changed lines of `path` in the work tree compared to `HEAD`.
    pub fn line_changes(&self, path: &Path) -> Result<Vec<LineChange>, Error> {
        let rela = self.relative(path).ok_or_else(|| err("path is outside the work tree"))?;
        lines::compute(&self.repo, &self.root, &rela)
    }
}

#[cfg(test)]
mod tests;
