//! Talking to the upstream: fetch, pull and push.

use super::*;

impl Repo {
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
            let branch =
                self.branch().ok_or_else(|| Error("HEAD is detached: nothing to push".into()))?;
            args.extend(["--set-upstream", "origin"]);
            return self.git_net(&[args.as_slice(), &[branch.as_str()]].concat()).map(|_| ());
        }
        self.git_net(&args).map(|_| ())
    }
}
