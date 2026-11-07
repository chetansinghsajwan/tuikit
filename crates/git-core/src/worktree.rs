//! Working-tree changes and the operations that change them (stage, unstage,
//! discard, commit). Reading goes through `gix`; writing runs the `git` command
//! line, as lazygit does, so hooks, signing and every edge case behave exactly as
//! the user's git does.

use crate::history::{FilePatch, RefKind, hunks};
use crate::lines::head_blob;
use crate::{Error, Kind, Repo, err};
use gix::bstr::ByteSlice;
use std::path::{Path, PathBuf};
use std::process::Command;

/// How far `Repo::reset` goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetMode {
    /// Keep the index and the work tree.
    Soft,
    /// Keep the work tree, reset the index.
    Mixed,
    /// Reset both: changes are lost.
    Hard,
}

/// An operation that stopped and waits for you (usually on conflicts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

/// Which comparison a file diff shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// `HEAD` against the index: what the next commit contains.
    Staged,
    /// The index against the work tree: what is not staged yet (an untracked
    /// file counts as all added).
    Unstaged,
}

impl Repo {
    fn index_blob(&self, rela: &Path) -> Option<Vec<u8>> {
        let index = self.repo.index_or_empty().ok()?;
        let entry = index.entry_by_path(rela.to_str()?.into())?;
        Some(self.repo.find_blob(entry.id).ok()?.detach().data)
    }

    /// The diff of one file (path relative to the work tree) on `side`.
    pub fn diff_file(&self, rela: &Path, side: Side) -> Result<FilePatch, Error> {
        let (before, after, kind) = match side {
            Side::Staged => {
                let before = head_blob(&self.repo, rela);
                let after = self.index_blob(rela);
                let kind = match (&before, &after) {
                    (None, _) => Kind::Added,
                    (_, None) => Kind::Deleted,
                    _ => Kind::Modified,
                };
                (before.unwrap_or_default(), after.unwrap_or_default(), kind)
            }
            Side::Unstaged => {
                let before = self.index_blob(rela);
                let after = std::fs::read(self.root.join(rela)).ok();
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
            path: rela.to_string_lossy().into_owned(),
            old_path: None,
            modes: None,
            kind,
            binary,
            hunks: if binary { Vec::new() } else { hunks(&before, &after) },
            before: crate::history::full_text(&before),
            after: crate::history::full_text(&after),
        })
    }

    /// The checked-out branch, or `None` on a detached HEAD.
    pub fn branch(&self) -> Option<String> {
        let name = self.repo.head_name().ok()??;
        Some(name.shorten().to_str_lossy().into_owned())
    }

    fn git(&self, args: &[&str], paths: &[PathBuf]) -> Result<String, Error> {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.root).args(["--no-pager", "-c", "core.quotepath=off"]).args(args);
        if !paths.is_empty() {
            cmd.arg("--").args(paths);
        }
        let out = cmd.env("GIT_TERMINAL_PROMPT", "0").env("GIT_EDITOR", "true").output().map_err(err)?;
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
        }
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let msg = if msg.is_empty() { String::from_utf8_lossy(&out.stdout).trim().to_string() } else { msg };
        Err(Error(msg.lines().last().unwrap_or("git failed").to_string()))
    }

    fn has_head(&self) -> bool {
        self.repo.head_commit().is_ok()
    }

    /// Stage changes (new, modified and deleted files) for `paths`.
    pub fn stage(&self, paths: &[PathBuf]) -> Result<(), Error> {
        self.git(&["add", "-A"], paths).map(|_| ())
    }

    /// Stage everything.
    pub fn stage_all(&self) -> Result<(), Error> {
        self.git(&["add", "-A"], &[]).map(|_| ())
    }

    /// Unstage `paths` (keeps the work tree as is).
    pub fn unstage(&self, paths: &[PathBuf]) -> Result<(), Error> {
        if self.has_head() {
            self.git(&["restore", "--staged"], paths).map(|_| ())
        } else {
            // No commit yet: unstaging means removing the files from the index.
            self.git(&["rm", "-r", "--cached", "-q"], paths).map(|_| ())
        }
    }

    pub fn unstage_all(&self) -> Result<(), Error> {
        if self.has_head() {
            self.git(&["reset", "-q"], &[]).map(|_| ())
        } else {
            self.git(&["rm", "-r", "--cached", "-q", "."], &[]).map(|_| ())
        }
    }

    /// Throw away work-tree changes: tracked files are restored from the index,
    /// untracked files are deleted. Not recoverable.
    pub fn discard(&self, rela: &Path, untracked: bool) -> Result<(), Error> {
        if untracked {
            let path = self.root.join(rela);
            let meta = path.symlink_metadata().map_err(err)?;
            return if meta.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) }.map_err(err);
        }
        self.git(&["restore"], &[rela.to_path_buf()]).map(|_| ())
    }

    /// Switch to a ref: a local branch, a remote branch (a tracking branch is created) or a
    /// tag (detached HEAD).
    pub fn checkout_ref(&self, name: &str, kind: crate::RefKind) -> Result<(), Error> {
        match kind {
            crate::RefKind::Branch => self.git(&["switch", name], &[]),
            crate::RefKind::Remote => self.git(&["switch", "--track", name], &[]),
            crate::RefKind::Tag => self.git(&["switch", "--detach", name], &[]),
        }
        .map(|_| ())
    }

    /// Create a branch at HEAD and switch to it.
    pub fn create_branch(&self, name: &str) -> Result<(), Error> {
        if name.trim().is_empty() {
            return Err(Error("empty branch name".into()));
        }
        self.git(&["switch", "-c", name], &[]).map(|_| ())
    }

    /// Delete a local branch; `force` deletes it even if it is not merged.
    pub fn delete_branch(&self, name: &str, force: bool) -> Result<(), Error> {
        self.git(&["branch", if force { "-D" } else { "-d" }, name], &[]).map(|_| ())
    }

    /// Merge `name` into the checked-out branch.
    pub fn merge(&self, name: &str) -> Result<(), Error> {
        self.git(&["merge", name], &[]).map(|_| ())
    }

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

    /// Like `git`, for commands that talk to a remote: never prompts (the TUI owns the
    /// terminal), and ssh runs in batch mode so a passphrase prompt cannot take over the screen
    /// (keys in an agent still work).
    fn git_net(&self, args: &[&str]) -> Result<String, Error> {
        let base = std::env::var("GIT_SSH_COMMAND").ok().or_else(|| {
            let out = Command::new("git")
                .current_dir(&self.root)
                .args(["config", "--get", "core.sshCommand"])
                .output()
                .ok()?;
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            (!s.is_empty()).then_some(s)
        });
        let ssh = format!("{} -o BatchMode=yes", base.unwrap_or_else(|| "ssh".into()));
        let out = Command::new("git")
            .current_dir(&self.root)
            .args(["--no-pager"])
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_EDITOR", "true")
            .env("GIT_SSH_COMMAND", ssh)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(err)?;
        if out.status.success() {
            return Ok(format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)));
        }
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(Error(msg.lines().last().unwrap_or("git failed").to_string()))
    }

    /// Whether the checked-out branch tracks a remote branch.
    pub fn has_upstream(&self) -> bool {
        Command::new("git")
            .current_dir(&self.root)
            .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Fetch every remote and prune what is gone there.
    pub fn fetch(&self) -> Result<(), Error> {
        self.git_net(&["fetch", "--all", "--prune"]).map(|_| ())
    }

    /// Pull the upstream into the checked-out branch (a fast-forward or a merge, as git is configured).
    pub fn pull(&self) -> Result<(), Error> {
        self.git_net(&["pull"]).map(|_| ())
    }

    /// Push the checked-out branch; the first push of a branch sets its upstream on `origin`.
    /// `force` uses `--force-with-lease`.
    pub fn push(&self, force: bool) -> Result<(), Error> {
        let mut args = vec!["push"];
        if force {
            args.push("--force-with-lease");
        }
        if !self.has_upstream() {
            let branch = self.branch().ok_or_else(|| Error("HEAD is detached: nothing to push".into()))?;
            args.extend(["--set-upstream", "origin"]);
            return self.git_net(&[args.as_slice(), &[branch.as_str()]].concat()).map(|_| ());
        }
        self.git_net(&args).map(|_| ())
    }

    /// Check out a commit (detached HEAD).
    pub fn checkout_commit(&self, oid: &str) -> Result<(), Error> {
        self.git(&["switch", "--detach", oid], &[]).map(|_| ())
    }

    /// Create a branch at `oid` and switch to it.
    pub fn create_branch_at(&self, name: &str, oid: &str) -> Result<(), Error> {
        if name.trim().is_empty() {
            return Err(Error("empty branch name".into()));
        }
        self.git(&["switch", "-c", name, oid], &[]).map(|_| ())
    }

    pub fn rename_branch(&self, old: &str, new: &str) -> Result<(), Error> {
        self.check_branch_name(new)?;
        self.git(&["branch", "-m", old, new], &[]).map(|_| ())
    }

    /// Whether `name` is usable as a branch name (`git check-ref-format`: no leading `-`, no `..`, no spaces, ...).
    pub fn check_branch_name(&self, name: &str) -> Result<(), Error> {
        if name.starts_with('-') {
            return Err(Error(format!("\"{name}\" is not a valid branch name")));
        }
        self.git(&["check-ref-format", &format!("refs/heads/{name}")], &[])
            .map(|_| ())
            .map_err(|_| Error(format!("\"{name}\" is not a valid branch name")))
    }

    /// Whether `name` is usable as a tag name.
    pub fn check_tag_name(&self, name: &str) -> Result<(), Error> {
        if name.starts_with('-') {
            return Err(Error(format!("\"{name}\" is not a valid tag name")));
        }
        self.git(&["check-ref-format", &format!("refs/tags/{name}")], &[])
            .map(|_| ())
            .map_err(|_| Error(format!("\"{name}\" is not a valid tag name")))
    }

    /// Rename a tag: a new tag at the same commit with the same message (an annotated tag is made again; its
    /// tagger and date are the rename's), then the old one goes. If the old one cannot go, the new one is removed again.
    pub fn rename_tag(&self, old: &str, new: &str) -> Result<(), Error> {
        self.check_tag_name(new)?;
        if old == new {
            return Err(Error("the new name is the old name".into()));
        }
        let full = format!("refs/tags/{old}");
        if self.git(&["cat-file", "-t", &full], &[])?.trim() == "tag" {
            let message = self.git(&["for-each-ref", "--format=%(contents)", &full], &[])?;
            self.git(&["tag", "-a", "-m", message.trim_end(), new, &format!("{full}^{{}}")], &[])?;
        } else {
            self.git(&["tag", new, &full], &[])?;
        }
        if let Err(e) = self.git(&["tag", "-d", old], &[]) {
            let _ = self.git(&["tag", "-d", new], &[]);
            return Err(e);
        }
        Ok(())
    }

    /// Rename several branches (or tags) at once, all or none: every new name is checked first (valid, not taken, not
    /// used twice), and if one rename fails the ones already done are undone.
    pub fn rename_many(&self, kind: RefKind, pairs: &[(String, String)]) -> Result<(), Error> {
        let rename = |old: &str, new: &str| {
            if kind == RefKind::Tag { self.rename_tag(old, new) } else { self.rename_branch(old, new) }
        };
        let taken: std::collections::HashSet<String> =
            self.refs()?.into_iter().filter(|r| r.kind == kind).map(|r| r.name).collect();
        let leaving: std::collections::HashSet<&str> = pairs.iter().map(|(old, _)| old.as_str()).collect();
        let mut news = std::collections::HashSet::new();
        for (_, new) in pairs {
            if kind == RefKind::Tag {
                self.check_tag_name(new)?
            } else {
                self.check_branch_name(new)?
            }
            if !news.insert(new.as_str()) {
                return Err(Error(format!("\"{new}\" would be used twice")));
            }
            if taken.contains(new) && !leaving.contains(new.as_str()) {
                return Err(Error(format!("\"{new}\" already exists")));
            }
        }
        for (i, (old, new)) in pairs.iter().enumerate() {
            if let Err(e) = rename(old, new) {
                for (old, new) in pairs[..i].iter().rev() {
                    let _ = rename(new, old);
                }
                return Err(e);
            }
        }
        Ok(())
    }

    /// Rename a remote (`git remote rename`): its branches, tracking settings and `origin/...` refs follow.
    pub fn rename_remote(&self, old: &str, new: &str) -> Result<(), Error> {
        if new.trim().is_empty() || new.starts_with('-') || new.contains(char::is_whitespace) {
            return Err(Error(format!("\"{new}\" is not a valid remote name")));
        }
        self.git(&["remote", "rename", old, new], &[]).map(|_| ())
    }

    /// Rename a branch on its remote (`origin/old` to `new`: push the commit under the new name, then delete the old one).
    pub fn rename_remote_branch(&self, remote_ref: &str, new: &str) -> Result<(), Error> {
        let (remote, old) =
            remote_ref.split_once('/').ok_or_else(|| Error(format!("{remote_ref} is not a remote branch")))?;
        if old == "HEAD" {
            return Err(Error("origin/HEAD is not a branch".into()));
        }
        self.check_branch_name(new)?;
        // The same name would push nothing and then delete the branch.
        if new == old {
            return Err(Error("the new name is the old name".into()));
        }
        self.git_net(&["push", remote, &format!("{remote_ref}:refs/heads/{new}")])?;
        self.git_net(&["push", remote, "--delete", old]).map(|_| ())
    }

    /// Delete a tag.
    pub fn delete_tag(&self, name: &str) -> Result<(), Error> {
        self.git(&["tag", "-d", name], &[]).map(|_| ())
    }

    /// Create a tag at `oid` (a lightweight one).
    pub fn create_tag(&self, name: &str, oid: &str) -> Result<(), Error> {
        if name.trim().is_empty() {
            return Err(Error("empty tag name".into()));
        }
        self.git(&["tag", name, oid], &[]).map(|_| ())
    }

    /// Delete a branch on its remote (`origin/topic` runs `git push origin --delete topic`).
    pub fn delete_remote_branch(&self, remote_ref: &str) -> Result<(), Error> {
        let (remote, branch) =
            remote_ref.split_once('/').ok_or_else(|| Error(format!("{remote_ref} is not a remote branch")))?;
        self.git_net(&["push", remote, "--delete", branch]).map(|_| ())
    }

    /// Apply the changes of a commit on top of HEAD.
    pub fn cherry_pick(&self, oid: &str) -> Result<(), Error> {
        self.git(&["cherry-pick", oid], &[]).map(|_| ())
    }

    /// Add a commit that undoes `oid`.
    pub fn revert(&self, oid: &str) -> Result<(), Error> {
        self.git(&["revert", "--no-edit", oid], &[]).map(|_| ())
    }

    /// Move the branch to `oid`: `Soft` keeps the index and the work tree, `Mixed` keeps only
    /// the work tree, `Hard` throws both away.
    pub fn reset(&self, oid: &str, mode: ResetMode) -> Result<(), Error> {
        let flag = match mode {
            ResetMode::Soft => "--soft",
            ResetMode::Mixed => "--mixed",
            ResetMode::Hard => "--hard",
        };
        self.git(&["reset", flag, oid], &[]).map(|_| ())
    }

    /// Replay the checked-out branch on top of `onto`.
    pub fn rebase(&self, onto: &str) -> Result<(), Error> {
        self.git(&["rebase", onto], &[]).map(|_| ())
    }

    /// Change the last commit: with a message, reword it (and take what is staged); without,
    /// add what is staged to it and keep the message.
    pub fn amend(&self, message: Option<&str>) -> Result<(), Error> {
        match message {
            Some(m) if m.trim().is_empty() => Err(Error("empty commit message".into())),
            Some(m) => self.git(&["commit", "-q", "--amend", "-m", m], &[]).map(|_| ()),
            None => self.git(&["commit", "-q", "--amend", "--no-edit"], &[]).map(|_| ()),
        }
    }

    /// Change only the message of the last commit; what is staged stays staged.
    pub fn reword(&self, message: &str) -> Result<(), Error> {
        if message.trim().is_empty() {
            return Err(Error("empty commit message".into()));
        }
        self.git(&["commit", "-q", "--amend", "--only", "-m", message], &[]).map(|_| ())
    }

    /// A commit holding the current state of the tracked files (staged and unstaged changes included),
    /// made without touching the branch, the index or the work tree; `HEAD` when nothing changed.
    pub fn worktree_snapshot(&self) -> Result<String, Error> {
        let out = self.git(&["stash", "create"], &[])?;
        let id = out.trim();
        if id.is_empty() { Ok(self.git(&["rev-parse", "HEAD"], &[])?.trim().to_string()) } else { Ok(id.to_string()) }
    }

    /// Change only the message of any commit on the checked-out history. The commits after it are
    /// rebuilt on the new one (same trees, authors and dates); the files and the index do not change.
    pub fn reword_commit(&self, oid: &str, message: &str) -> Result<(), Error> {
        if message.trim().is_empty() {
            return Err(Error("empty commit message".into()));
        }
        let head = self.git(&["rev-parse", "HEAD"], &[])?.trim().to_string();
        let target = self.git(&["rev-parse", "--verify", &format!("{oid}^{{commit}}")], &[])?.trim().to_string();
        if target == head {
            return self.reword(message);
        }
        self.git(&["merge-base", "--is-ancestor", &target, &head], &[])
            .map_err(|_| Error("that commit is not on the checked-out history".into()))?;
        let mut chain = vec![target.clone()];
        chain.extend(
            self.git(&["rev-list", "--reverse", "--topo-order", "--ancestry-path", &format!("{target}..{head}")], &[])?
                .lines()
                .map(str::to_string),
        );
        let mut done: std::collections::HashMap<String, String> = Default::default();
        for c in &chain {
            let info =
                self.git(&["show", "-s", "--date=raw", "--format=%T%n%P%n%an%n%ae%n%ad%n%cn%n%ce%n%cd", c], &[])?;
            let mut f = info.lines();
            let mut next = || f.next().unwrap_or("").to_string();
            let (tree, parents) = (next(), next());
            let (an, ae, ad, cn, ce, cd) = (next(), next(), next(), next(), next(), next());
            let body = if *c == target {
                message.to_string()
            } else {
                self.git(&["show", "-s", "--format=%B", c], &[])?.trim_end().to_string()
            };
            let mut cmd = Command::new("git");
            cmd.current_dir(&self.root).args(["commit-tree", &tree]);
            for p in parents.split_whitespace() {
                cmd.arg("-p").arg(done.get(p).map(String::as_str).unwrap_or(p));
            }
            let out = cmd
                .args(["-m", &body])
                .env("GIT_AUTHOR_NAME", an)
                .env("GIT_AUTHOR_EMAIL", ae)
                .env("GIT_AUTHOR_DATE", ad)
                .env("GIT_COMMITTER_NAME", cn)
                .env("GIT_COMMITTER_EMAIL", ce)
                .env("GIT_COMMITTER_DATE", cd)
                .output()
                .map_err(err)?;
            if !out.status.success() {
                return Err(Error(
                    String::from_utf8_lossy(&out.stderr)
                        .trim()
                        .lines()
                        .last()
                        .unwrap_or("commit-tree failed")
                        .to_string(),
                ));
            }
            done.insert(c.clone(), String::from_utf8_lossy(&out.stdout).trim().to_string());
        }
        let new = &done[chain.last().unwrap()];
        self.git(&["update-ref", "-m", "reword", "HEAD", new, &head], &[]).map(|_| ())
    }

    /// The operation under way that stopped (a conflict, say), if any.
    pub fn operation(&self) -> Option<Operation> {
        let dir = self.git_dir()?;
        if dir.join("rebase-merge").exists() || dir.join("rebase-apply").exists() {
            Some(Operation::Rebase)
        } else if dir.join("MERGE_HEAD").exists() {
            Some(Operation::Merge)
        } else if dir.join("CHERRY_PICK_HEAD").exists() {
            Some(Operation::CherryPick)
        } else if dir.join("REVERT_HEAD").exists() {
            Some(Operation::Revert)
        } else {
            None
        }
    }

    fn git_dir(&self) -> Option<PathBuf> {
        let out =
            Command::new("git").current_dir(&self.root).args(["rev-parse", "--absolute-git-dir"]).output().ok()?;
        out.status.success().then(|| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
    }

    /// Continue the stopped operation once the conflicts are resolved and staged.
    pub fn continue_operation(&self) -> Result<(), Error> {
        match self.operation() {
            Some(Operation::Rebase) => self.git(&["rebase", "--continue"], &[]),
            Some(Operation::Merge) => self.git(&["merge", "--continue"], &[]),
            Some(Operation::CherryPick) => self.git(&["cherry-pick", "--continue"], &[]),
            Some(Operation::Revert) => self.git(&["revert", "--continue"], &[]),
            None => Err(Error("nothing to continue".into())),
        }
        .map(|_| ())
    }

    /// Give up the stopped operation and go back to how things were.
    pub fn abort_operation(&self) -> Result<(), Error> {
        match self.operation() {
            Some(Operation::Rebase) => self.git(&["rebase", "--abort"], &[]),
            Some(Operation::Merge) => self.git(&["merge", "--abort"], &[]),
            Some(Operation::CherryPick) => self.git(&["cherry-pick", "--abort"], &[]),
            Some(Operation::Revert) => self.git(&["revert", "--abort"], &[]),
            None => Err(Error("nothing to abort".into())),
        }
        .map(|_| ())
    }

    /// Commit what is staged. Returns `short-hash subject`.
    pub fn create_commit(&self, message: &str) -> Result<String, Error> {
        if message.trim().is_empty() {
            return Err(Error("empty commit message".into()));
        }
        self.git(&["commit", "-q", "-m", message], &[])?;
        let head = self.repo.head_commit().map_err(err)?;
        let id = head.id.to_string();
        Ok(format!("{} {}", &id[..id.len().min(8)], message.lines().next().unwrap_or("")))
    }
}
