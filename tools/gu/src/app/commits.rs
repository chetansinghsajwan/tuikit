//! Commits: their actions, prompts and questions.

use super::*;

impl App {
    /// The actions that act on the highlighted commit.
    pub(super) fn commit_action(&mut self, id: &str) {
        let Some(k) = self.commits.get(self.lists[1].cursor).cloned() else { return };
        match id {
            "commit.copy_hash" => match ui::clipboard::copy(&k.oid) {
                Ok(()) => self.show_message(format!("copied {} to the clipboard", k.short), false),
                Err(e) => self.show_message(format!("could not copy: {e}"), true),
            },
            "commit.files" => self.open_commit_files(),
            "commit.compare_work" => {
                self.work = !self.work;
                self.compare = None;
                self.detail = true;
                self.show_message(
                    if self.work {
                        "diff from the commit to the work tree (M or Esc clears)"
                    } else {
                        "compare cleared"
                    },
                    false,
                );
            }
            "commit.compare" => {
                self.work = false;
                if self.compare.as_deref() == Some(k.oid.as_str()) {
                    self.compare = None;
                    self.show_message("compare cleared", false);
                } else {
                    self.compare = Some(k.oid.clone());
                    self.detail = true;
                    self.show_message(
                        format!("comparing with {}: move to another commit (m clears)", k.short),
                        false,
                    );
                }
            }
            "commit.checkout" => {
                let oid = k.oid.clone();
                self.run(format!("checked out {}", k.short), move |r| r.checkout_commit(&oid));
            }
            "commit.cherry_pick" => {
                let oid = k.oid.clone();
                self.run(format!("cherry-picked {}", k.short), move |r| r.cherry_pick(&oid));
            }
            "commit.revert" => self.mode = Mode::Confirm(Confirm::Revert(k.oid)),
            "commit.reword" => {
                self.mode = Mode::Prompt(PromptKind::Reword(k.oid), Input::new(k.subject));
            }
            "commit.amend" => {
                let head = self.commits.first().map(|c| c.subject.clone()).unwrap_or_default();
                self.mode = Mode::Prompt(PromptKind::Amend(head.clone()), Input::new(head));
            }
            "commit.branch" => {
                self.mode = Mode::Prompt(PromptKind::BranchAt(k.oid), Input::default())
            }
            "commit.tag" => self.mode = Mode::Prompt(PromptKind::TagAt(k.oid), Input::default()),
            "commit.reset_soft" => self.run(format!("reset (soft) to {}", k.short), move |r| {
                r.reset(&k.oid, ResetMode::Soft)
            }),
            "commit.reset_mixed" => self.run(format!("reset (mixed) to {}", k.short), move |r| {
                r.reset(&k.oid, ResetMode::Mixed)
            }),
            "commit.reset_hard" => {
                self.mode = Mode::Confirm(Confirm::Reset { oid: k.oid, mode: ResetMode::Hard })
            }
            _ => {}
        }
    }

    /// Run a finished prompt about a commit.
    pub(super) fn commit_submit(&mut self, kind: PromptKind, value: String) {
        match kind {
            PromptKind::BranchAt(oid) => {
                let v = value.clone();
                self.run(format!("created {value}"), move |r| r.create_branch_at(&v, &oid));
            }
            PromptKind::TagAt(oid) => {
                let v = value.clone();
                self.run(format!("tagged {value}"), move |r| r.create_tag(&v, &oid));
            }
            PromptKind::Reword(oid) => {
                let v = value.clone();
                self.run("reworded".to_string(), move |r| r.reword_commit(&oid, &v));
            }
            PromptKind::Amend(old) => {
                let v = value.clone();
                self.run("amended the last commit".to_string(), move |r| {
                    if v == old { r.amend(None) } else { r.amend(Some(&v)) }
                });
            }
            _ => {}
        }
    }

    /// Run a confirmed change to the history.
    pub(super) fn commit_confirmed(&mut self, c: Confirm) {
        match c {
            Confirm::Revert(oid) => self.run("reverted".to_string(), move |r| r.revert(&oid)),
            Confirm::Reset { oid, mode } => {
                self.run("reset (hard)".to_string(), move |r| r.reset(&oid, mode))
            }
            _ => {}
        }
    }
}
