//! Moving history: merge, cherry-pick, revert, reset and rebase.

use super::*;

impl Repo {
    /// Merge `name` into the checked-out branch.
    pub fn merge(&self, name: &str) -> Result<(), Error> {
        self.git(&["merge", name], &[]).map(|_| ())
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
}
