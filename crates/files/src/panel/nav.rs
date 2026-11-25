//! Movement: the cursor keys and folding.

use super::*;

impl Panel {
    /// Runs one of this domain's actions by id.
    pub(super) fn run_nav_action(&mut self, id: &str) -> Outcome {
        if !ui::app::run_nav(&mut self.list, id) && id == "tree.collapse" {
            self.left();
        }
        Outcome::None
    }
}
