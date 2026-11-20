//! Finding files and contents: the host runs the picker and reveals the result.

use super::*;
use ui::keys::{Action as Key, action};

/// The actions of this domain with their default keys.
pub(super) const ACTIONS: &[Key] = &[
    action(
        "search.here",
        "find here",
        "find names in the folder under the cursor only, not in its subfolders (the picker)",
        &["/"],
    ),
    action(
        "search.files",
        "find everywhere",
        "find files by name in the root and every subfolder (the picker)",
        &["leader f"],
    ),
    action("search.grep", "grep", "search file contents (the picker)", &["leader s"]),
];

impl Panel {
    /// Runs one of this domain's actions by id.
    pub(super) fn run_search(&mut self, id: &str) -> Action {
        match id {
            "search.files" => return Action::Search(Search::Files),
            "search.here" => return Action::Search(Search::Here(self.target_dir())),
            "search.grep" => return Action::Search(Search::Grep),
            _ => {}
        }
        Action::None
    }
}
