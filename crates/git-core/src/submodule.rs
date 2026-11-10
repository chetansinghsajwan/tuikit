//! Submodules: listing with their state, and the changes (init, update, sync, add, remove).
//! Reads and changes go through the `git` command line, which knows `.gitmodules`, the index
//! and the submodule's own work tree.

use crate::{Error, Repo};
use std::path::PathBuf;
use std::process::Command;

/// Where a submodule stands against what the super-project records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmoduleState {
    /// Not cloned yet (`git submodule update --init` fetches it).
    Uninitialized,
    /// At the recorded commit.
    Clean,
    /// At another commit than the recorded one (a pointer change to stage).
    Moved,
    /// Merge conflict on the recorded commit.
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmoduleInfo {
    /// The name in `.gitmodules` (usually the path).
    pub name: String,
    /// Path from the work tree root.
    pub path: String,
    pub url: String,
    /// The commit now checked out (or the recorded one when not cloned).
    pub oid: String,
    /// `git describe` of that commit, when there is one.
    pub describe: String,
    pub state: SubmoduleState,
    /// The submodule's own work tree has uncommitted changes.
    pub dirty: bool,
}

impl Repo {
    /// Every submodule of the repo, sorted by path.
    pub fn submodules(&self) -> Result<Vec<SubmoduleInfo>, Error> {
        if !self.root.join(".gitmodules").is_file() {
            return Ok(Vec::new());
        }
        // `name path` and `name url` pairs from `.gitmodules`.
        let cfg = self
            .git(
                &[
                    "config",
                    "-f",
                    ".gitmodules",
                    "--null",
                    "--get-regexp",
                    r"^submodule\..*\.(path|url)$",
                ],
                &[],
            )
            .unwrap_or_default(); // git exits 1 when nothing matches
        let mut names: std::collections::HashMap<String, (String, String)> = Default::default();
        for entry in cfg.split('\0').filter(|e| !e.is_empty()) {
            let Some((key, value)) = entry.split_once('\n') else { continue };
            let Some(rest) = key.strip_prefix("submodule.") else { continue };
            let Some((name, field)) = rest.rsplit_once('.') else { continue };
            let slot = names.entry(name.to_string()).or_default();
            match field {
                "path" => slot.0 = value.to_string(),
                "url" => slot.1 = value.to_string(),
                _ => {}
            }
        }
        let by_path: std::collections::HashMap<&str, (&String, &String)> =
            names.iter().map(|(n, (p, u))| (p.as_str(), (n, u))).collect();
        let status = self.git(&["submodule", "status"], &[])?;
        let mut out = Vec::new();
        for line in status.lines() {
            let Some(mark) = line.chars().next() else { continue };
            let mut parts = line[mark.len_utf8()..].split_whitespace();
            let (Some(oid), Some(path)) = (parts.next(), parts.next()) else { continue };
            let describe = parts.collect::<Vec<_>>().join(" ").trim_matches(['(', ')']).to_string();
            let state = match mark {
                '-' => SubmoduleState::Uninitialized,
                '+' => SubmoduleState::Moved,
                'U' => SubmoduleState::Conflict,
                _ => SubmoduleState::Clean,
            };
            let (name, url) =
                by_path.get(path).map(|(n, u)| ((*n).clone(), (*u).clone())).unwrap_or_default();
            let dirty = state != SubmoduleState::Uninitialized
                && Command::new("git")
                    .current_dir(self.root.join(path))
                    .args(["status", "--porcelain", "--ignore-submodules=none"])
                    .output()
                    .map(|o| !o.stdout.is_empty())
                    .unwrap_or(false);
            out.push(SubmoduleInfo {
                name: if name.is_empty() { path.to_string() } else { name },
                path: path.to_string(),
                url,
                oid: oid.to_string(),
                describe,
                state,
                dirty,
            });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    /// What the highlighted submodule changed: the commits between the recorded and the checked-out one.
    pub fn submodule_summary(&self, path: &str) -> Result<String, Error> {
        let summary = self.git(&["submodule", "summary", "--", path], &[])?;
        if !summary.trim().is_empty() {
            return Ok(summary);
        }
        self.git(&["submodule", "status", "--", path], &[])
    }

    /// Clone (if needed) and check out the recorded commit of `path`, nested submodules too.
    pub fn submodule_update(&self, path: &str) -> Result<(), Error> {
        self.git_net(&["submodule", "update", "--init", "--recursive", "--", path]).map(|_| ())
    }

    /// The same for every submodule.
    pub fn submodule_update_all(&self) -> Result<(), Error> {
        self.git_net(&["submodule", "update", "--init", "--recursive"]).map(|_| ())
    }

    /// Copy the URLs of `.gitmodules` to the config of the repo and of each submodule.
    pub fn submodule_sync(&self) -> Result<(), Error> {
        self.git(&["submodule", "sync", "--recursive"], &[]).map(|_| ())
    }

    /// Check out the tip of the submodule's tracked remote branch (`update --remote`).
    pub fn submodule_update_remote(&self, path: &str) -> Result<(), Error> {
        self.git_net(&["submodule", "update", "--init", "--remote", "--", path]).map(|_| ())
    }

    /// Add the repository at `url` as a submodule at `path`.
    pub fn submodule_add(&self, url: &str, path: &str) -> Result<(), Error> {
        self.git_net(&["submodule", "add", "--", url, path]).map(|_| ())
    }

    /// Remove a submodule: deinit it, drop it from the index and `.gitmodules`, and delete its work tree.
    pub fn submodule_remove(&self, path: &str) -> Result<(), Error> {
        self.git(&["submodule", "deinit", "-f", "--", path], &[])?;
        self.git(&["rm", "-f", "--", path], &[])?;
        // The cloned repo is kept in `.git/modules`; it would block adding the path again.
        if let Ok(dir) = self.git(&["rev-parse", "--git-path", &format!("modules/{path}")], &[]) {
            let dir = PathBuf::from(dir.trim());
            let dir = if dir.is_absolute() { dir } else { self.root.join(dir) };
            let _ = std::fs::remove_dir_all(dir);
        }
        Ok(())
    }
}
