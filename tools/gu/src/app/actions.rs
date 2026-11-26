//! Running an action by id: the shared dispatch. Each feature handles its own ids.

use super::*;

impl App {
    /// Run an action by id: what a key, the leader menu or the palette resolves to.
    pub fn run_id(&mut self, id: &'static str) -> Outcome {
        if self.shows_commit_files() && self.commit_files_action(id) {
            return Outcome::Continue;
        }
        match id {
            "quit" => return Outcome::Quit,
            "repo.up" => self.repo_up(),
            "back" => {
                // Esc: clear a search first, then go back to the Status tab.
                if self.work {
                    self.work = false;
                } else if self.compare.is_some() {
                    self.compare = None;
                } else if self.search.is_some() {
                    self.search = None;
                } else {
                    self.select_tab(Tab::Status);
                }
            }
            _ => {
                let handled = self.search_action(id)
                    || self.filter_action(id)
                    || self.focus_action(id)
                    || self.switch_action(id)
                    || self.remote_action(id)
                    || self.operation_action(id)
                    || self.view_action(id)
                    || self.preview_action(id);
                if !handled {
                    self.pane_action(id);
                }
            }
        }
        Outcome::Continue
    }

    /// The actions that act on what is highlighted in the focused pane.
    pub(super) fn pane_action(&mut self, id: &str) {
        match self.focus {
            Focus::Branches => self.branch_action(id),
            Focus::Commits => self.commit_action(id),
            Focus::Stashes => self.stash_action(id),
            Focus::Submodules => self.submodule_action(id),
            _ => {}
        }
    }

    /// Turn what the Files panel asked for into a reload, a screen change or a `Outcome` for the host.
    pub(super) fn files_action(&mut self, action: PanelOutcome) -> Outcome {
        match action {
            PanelOutcome::Close => Outcome::Quit,
            PanelOutcome::Open(path) => Outcome::Edit(path),
            PanelOutcome::Changed => {
                self.reload_history();
                Outcome::Continue
            }
            PanelOutcome::Palette => Outcome::Palette,
            PanelOutcome::Help => Outcome::Help,
            PanelOutcome::Command(id) => self.run_id(id),
            PanelOutcome::None => Outcome::Continue,
        }
    }
}
