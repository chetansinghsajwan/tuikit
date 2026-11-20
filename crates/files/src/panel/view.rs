//! What the list shows: hidden and ignored files, the columns, tables as text, refresh.

use super::*;
use ui::keys::{Action as Key, action};

/// The actions of this domain with their default keys.
pub(super) const ACTIONS: &[Key] = &[
    action("view.hidden", "hidden files", "show or hide dot files", &[".", "leader v h"]),
    action(
        "view.ignored",
        "ignored files",
        "show or hide the entries .gitignore and the other ignore files hide",
        &["I", "leader v I"],
    ),
    action(
        "view.columns",
        "columns",
        "cycle the columns: permissions, size and time / size and time / permissions and time / off",
        &["leader v c"],
    ),
    action(
        "view.refresh",
        "refresh",
        "reread the folders and the git status",
        &["R", "leader v r"],
    ),
    action(
        "view.raw",
        "tables as text",
        "show CSV, TSV and spreadsheets in the preview as text instead of tables (and back)",
        &["leader v t"],
    ),
];

impl Panel {
    /// Runs one of this domain's actions by id.
    pub(super) fn run_view_action(&mut self, id: &str) -> Outcome {
        match id {
            "view.ignored" => {
                self.show_ignored = !self.show_ignored;
                self.rebuild();
            }
            "view.hidden" => {
                self.opts.hidden = !self.opts.hidden;
                self.rebuild();
            }
            "view.raw" => {
                pv::set_raw_tables(!pv::raw_tables());
                self.show_message(
                    if pv::raw_tables() { "tables show as text" } else { "tables show as tables" },
                    false,
                );
            }
            "view.columns" => {
                self.opts.linemode = self.opts.linemode.next();
                self.show_message(self.opts.linemode.label(), false);
            }
            "view.refresh" => self.refresh(),
            _ => {}
        }
        Outcome::None
    }
}
