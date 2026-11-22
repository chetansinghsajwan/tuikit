//! Finding files and contents: the host runs the picker and reveals the result.

use super::*;

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
