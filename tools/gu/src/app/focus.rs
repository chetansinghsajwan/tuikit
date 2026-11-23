//! Which pane has the focus.

use super::*;

impl App {
    /// Focus `focus` and show its preview from the top.
    pub(super) fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
        self.preview_scroll.reset();
    }

    /// Move the focus to the next or previous pane of the tab (only the Refs tab has two).
    pub(super) fn focus_by(&mut self, delta: isize) {
        let panes: &[Focus] = match self.tab {
            Tab::Status => &STATUS_PANES,
            Tab::Commits => &COMMITS_PANES,
            Tab::Refs => &REFS_PANES,
        };
        let at = panes.iter().position(|f| *f == self.focus).unwrap_or(0) as isize;
        self.focus = panes[(at + delta).rem_euclid(panes.len() as isize) as usize];
        self.preview_scroll.reset();
    }

    /// The actions that move the focus.
    pub(super) fn focus_action(&mut self, id: &str) -> bool {
        match id {
            "focus.next" => self.focus_by(1),
            "focus.prev" => self.focus_by(-1),
            "focus.files" => self.show(Focus::Files),
            "focus.branches" => self.show(Focus::Branches),
            "focus.commits" => self.show(Focus::Commits),
            "focus.stashes" => self.show(Focus::Stashes),
            "focus.submodules" => self.show(Focus::Submodules),
            _ => return false,
        }
        true
    }
}
