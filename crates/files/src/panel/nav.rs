//! Movement: the cursor keys and folding.

use super::*;
use ui::keys::{Action as Key, basic};

/// The actions of this domain with their default keys.
pub(super) const ACTIONS: &[Key] = &[
    basic("nav.up", "up", "move the cursor up", &["k", "up", "ctrl-p"]),
    basic("nav.down", "down", "move the cursor down", &["j", "down", "ctrl-n"]),
    basic("nav.up5", "up 5", "move the cursor up five rows", &["ctrl-k"]),
    basic("nav.down5", "down 5", "move the cursor down five rows", &["ctrl-j"]),
    basic("nav.page_up", "page up", "move a page up", &["pageup"]),
    basic("nav.page_down", "page down", "move a page down", &["pagedown"]),
    basic("nav.top", "top", "go to the first row", &["g", "home"]),
    basic("nav.bottom", "bottom", "go to the last row", &["G", "end"]),
    basic(
        "tree.out",
        "collapse",
        "collapse the folder, or go to the parent folder's row",
        &["h", "left"],
    ),
];

impl Panel {
    /// Runs one of this domain's actions by id.
    pub(super) fn run_nav(&mut self, id: &str) -> Action {
        match id {
            "nav.up" => self.move_by(-1),
            "nav.down" => self.move_by(1),
            "nav.up5" => self.move_by(-5),
            "nav.down5" => self.move_by(5),
            "nav.page_up" => self.move_by(-(self.page as isize)),
            "nav.page_down" => self.move_by(self.page as isize),
            "nav.top" => self.cursor = 0,
            "nav.bottom" => self.cursor = self.rows.len().saturating_sub(1),
            "tree.out" => self.left(),
            _ => {}
        }
        Action::None
    }
}
