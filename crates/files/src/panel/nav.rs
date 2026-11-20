//! Movement: the cursor keys and folding.

use super::*;
use ui::keys::{Action as Key, basic};

/// The actions of this domain with their default keys.
pub(super) const ACTIONS: &[Key] = &[
    basic("nav.up", "up", "move the cursor up", &["k", "up", "ctrl-p"]),
    basic("nav.down", "down", "move the cursor down", &["j", "down", "ctrl-n"]),
    basic("nav.up_step", "up a step", "move the cursor up several rows", &["ctrl-k"]),
    basic("nav.down_step", "down a step", "move the cursor down several rows", &["ctrl-j"]),
    basic("nav.page_up", "page up", "move a page up", &["pageup"]),
    basic("nav.page_down", "page down", "move a page down", &["pagedown"]),
    basic("nav.top", "top", "go to the first row", &["g", "home"]),
    basic("nav.bottom", "bottom", "go to the last row", &["G", "end"]),
    basic(
        "tree.collapse",
        "collapse",
        "collapse the folder, or go to the parent folder's row",
        &["h", "left"],
    ),
];

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
