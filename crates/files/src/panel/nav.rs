//! Movement: the cursor keys and folding.

use super::*;

impl Panel {
    /// Runs one of this domain's actions by id.
    pub(super) fn run_nav_action(&mut self, id: &str) -> Outcome {
        match id {
            "nav.up" => self.move_by(-1),
            "nav.down" => self.move_by(1),
            "nav.up_step" => self.move_by(-5),
            "nav.down_step" => self.move_by(5),
            "nav.page_up" => self.move_by(-(self.page as isize)),
            "nav.page_down" => self.move_by(self.page as isize),
            "nav.top" => self.cursor = 0,
            "nav.bottom" => self.cursor = self.rows.len().saturating_sub(1),
            "tree.collapse" => self.left(),
            _ => {}
        }
        Outcome::None
    }
}
