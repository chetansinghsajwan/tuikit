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
    pub(super) fn run_search_action(&mut self, id: &str) -> Outcome {
        match id {
            "search.files" => return Outcome::Search(Search::Files),
            "search.here" => return Outcome::Search(Search::Here(self.target_dir())),
            "search.grep" => return Outcome::Search(Search::Grep),
            _ => {}
        }
        Outcome::None
    }
}
