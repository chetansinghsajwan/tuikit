//! The operation that stopped and waits: read it, continue it or abort it.

use super::*;

impl Repo {
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

    /// The absolute path of the `.git` directory, where the in-progress operation files live.
    pub(super) fn git_dir(&self) -> Option<PathBuf> {
        let out = Command::new("git")
            .current_dir(&self.root)
            .args(["rev-parse", "--absolute-git-dir"])
            .output()
            .ok()?;
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
}
