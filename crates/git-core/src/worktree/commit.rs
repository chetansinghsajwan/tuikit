//! Making and rewriting commits.

use super::*;

impl Repo {
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
        if id.is_empty() {
            Ok(self.git(&["rev-parse", "HEAD"], &[])?.trim().to_string())
        } else {
            Ok(id.to_string())
        }
    }

    /// Change only the message of any commit on the checked-out history. The commits after it are
    /// rebuilt on the new one (same trees, authors and dates); the files and the index do not change.
    pub fn reword_commit(&self, oid: &str, message: &str) -> Result<(), Error> {
        if message.trim().is_empty() {
            return Err(Error("empty commit message".into()));
        }
        let head = self.git(&["rev-parse", "HEAD"], &[])?.trim().to_string();
        let target = self
            .git(&["rev-parse", "--verify", &format!("{oid}^{{commit}}")], &[])?
            .trim()
            .to_string();
        if target == head {
            return self.reword(message);
        }
        self.git(&["merge-base", "--is-ancestor", &target, &head], &[])
            .map_err(|_| Error("that commit is not on the checked-out history".into()))?;
        let mut chain = vec![target.clone()];
        chain.extend(
            self.git(
                &[
                    "rev-list",
                    "--reverse",
                    "--topo-order",
                    "--ancestry-path",
                    &format!("{target}..{head}"),
                ],
                &[],
            )?
            .lines()
            .map(str::to_string),
        );
        let mut done: std::collections::HashMap<String, String> = Default::default();
        for c in &chain {
            let info = self.git(
                &["show", "-s", "--date=raw", "--format=%T%n%P%n%an%n%ae%n%ad%n%cn%n%ce%n%cd", c],
                &[],
            )?;
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
        let new = &done[chain.last().expect("the chain starts with the target")];
        self.git(&["update-ref", "-m", "reword", "HEAD", new, &head], &[]).map(|_| ())
    }
}
