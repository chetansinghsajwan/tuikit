//! Which folder is the root of the tree.

use super::*;

impl Panel {
    /// Runs one of this domain's actions by id.
    pub(super) fn run_root_action(&mut self, id: &str) -> Outcome {
        match id {
            "root.into" => {
                if let Some(row) = self.rows.get(self.list.cursor()).filter(|r| r.is_dir) {
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
        Outcome::None
    }
}
