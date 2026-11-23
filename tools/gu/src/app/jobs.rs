//! Remote commands on their own thread, and the state the status line shows.

use super::*;

impl App {
    /// Run a git command that talks to a remote on its own thread; the screen shows `label …`
    /// and `tick` finishes it. One at a time.
    pub(super) fn remote(
        &mut self,
        label: &str,
        done: &str,
        f: impl FnOnce(&Repo) -> Result<(), git_core::Error> + Send + 'static,
    ) {
        if self.job.is_some() {
            self.show_message("busy: wait for the current remote command", true);
            return;
        }
        let root = self.root.clone();
        self.job = Some(Job::spawn(label, done, move || match Repo::discover(&root) {
            Some(repo) => f(&repo),
            None => Err(git_core::Error("the repository is gone".into())),
        }));
    }

    /// Call between events: finishes a remote command that ended. `true` when the screen should be redrawn.
    pub fn tick(&mut self) -> bool {
        if self.watch.as_ref().is_some_and(|w| w.take()) {
            self.reload_all();
            return true;
        }
        if let Some(done) = ui::job::finished(&mut self.job) {
            match done.result {
                Some(Ok(())) => self.show_message(done.done_text, false),
                Some(Err(e)) => self.show_message(e.to_string(), true),
                None => self.show_message("the remote command crashed", true),
            }
            self.reload_all();
            return true;
        }
        self.job.is_some()
    }

    /// The operation that stopped and waits (conflicts), shown in the status line.
    pub(super) fn operation_name(&self) -> Option<&'static str> {
        match self.repo.operation()? {
            Operation::Merge => Some("MERGING"),
            Operation::Rebase => Some("REBASING"),
            Operation::CherryPick => Some("CHERRY-PICKING"),
            Operation::Revert => Some("REVERTING"),
        }
    }

    /// Continue or abort the operation that stopped.
    pub(super) fn operation_action(&mut self, id: &str) -> bool {
        match id {
            "op.continue" => self.run("continued".to_string(), |r| r.continue_operation()),
            "op.abort" => self.run("aborted".to_string(), |r| r.abort_operation()),
            _ => return false,
        }
        true
    }
}
