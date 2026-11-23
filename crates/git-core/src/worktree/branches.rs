//! Branches, tags and remote branches: switch, create, delete and rename.

use super::*;

impl Repo {
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

    /// Check out a commit (detached HEAD).
    pub fn checkout_commit(&self, oid: &str) -> Result<(), Error> {
        self.git(&["switch", "--detach", oid], &[]).map(|_| ())
    }

    /// Create a branch at HEAD and switch to it.
    pub fn create_branch(&self, name: &str) -> Result<(), Error> {
        if name.trim().is_empty() {
            return Err(Error("empty branch name".into()));
        }
        self.git(&["switch", "-c", name], &[]).map(|_| ())
    }

    /// Create a branch at `oid` and switch to it.
    pub fn create_branch_at(&self, name: &str, oid: &str) -> Result<(), Error> {
        if name.trim().is_empty() {
            return Err(Error("empty branch name".into()));
        }
        self.git(&["switch", "-c", name, oid], &[]).map(|_| ())
    }

    /// Delete a local branch; `force` deletes it even if it is not merged.
    pub fn delete_branch(&self, name: &str, force: bool) -> Result<(), Error> {
        self.git(&["branch", if force { "-D" } else { "-d" }, name], &[]).map(|_| ())
    }

    /// Rename a local branch after checking the new name is valid.
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
            if kind == RefKind::Tag {
                self.rename_tag(old, new)
            } else {
                self.rename_branch(old, new)
            }
        };
        let taken: std::collections::HashSet<String> =
            self.refs()?.into_iter().filter(|r| r.kind == kind).map(|r| r.name).collect();
        let leaving: std::collections::HashSet<&str> =
            pairs.iter().map(|(old, _)| old.as_str()).collect();
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
        let (remote, old) = remote_ref
            .split_once('/')
            .ok_or_else(|| Error(format!("{remote_ref} is not a remote branch")))?;
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
        let (remote, branch) = remote_ref
            .split_once('/')
            .ok_or_else(|| Error(format!("{remote_ref} is not a remote branch")))?;
        self.git_net(&["push", remote, "--delete", branch]).map(|_| ())
    }
}
