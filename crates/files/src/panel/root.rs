//! Which folder is the root of the tree.

use super::*;
use ui::keys::{Action as Key, action};

/// The actions of this domain with their default keys.
pub(super) const ACTIONS: &[Key] = &[
    action("root.into", "cd into", "make the folder under the cursor the root", &["leader c l"]),
    action(
        "root.focus",
        "focus folder",
        "make the folder the cursor row sits in the root, so only its own entries show",
        &["leader c f"],
    ),
    action("root.up", "cd up", "make the parent folder the root", &["leader c h"]),
];

impl Panel {
    /// Runs one of this domain's actions by id.
    pub(super) fn run_root(&mut self, id: &str) -> Action {
        match id {
            "root.into" => {
                if let Some(row) = self.rows.get(self.cursor).filter(|r| r.is_dir) {
                    let dir = row.path.clone();
                    self.set_root(dir);
                }
            }
            "root.focus" => {
                // The folder the cursor row sits in becomes the root: its siblings only, no other branches.
                if let Some(path) = self.current().map(Path::to_path_buf)
                    && let Some(dir) = path.parent().map(Path::to_path_buf)
                {
                    self.set_root(dir);
                    self.reveal(&path);
                }
            }
            "root.up" => {
                if let Some(parent) = self.root.parent().map(Path::to_path_buf) {
                    let old = self.root.clone();
                    self.set_root(parent);
                    self.reveal(&old);
                }
            }
            _ => {}
        }
        Action::None
    }
}
