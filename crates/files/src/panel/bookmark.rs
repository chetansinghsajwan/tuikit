//! Bookmarks: folders saved under a key.

use super::*;
use ui::keys::{Action as Key, action};

/// The actions of this domain with their default keys.
pub(super) const ACTIONS: &[Key] = &[
    action(
        "bookmark.set",
        "save bookmark",
        "save the folder under the cursor under a key you type",
        &["leader b s"],
    ),
    action("bookmark.jump", "go to bookmark", "make a bookmarked folder the root", &["leader b j"]),
    action("bookmark.delete", "delete bookmark", "delete a bookmark", &["leader b d"]),
];

impl Panel {
    /// Runs one of this domain's actions by id.
    pub(super) fn run_bookmark(&mut self, id: &str) -> Outcome {
        match id {
            "bookmark.set" => self.mode = Mode::Bookmark(BookmarkOp::Set),
            "bookmark.jump" => self.mode = Mode::Bookmark(BookmarkOp::Jump),
            "bookmark.delete" => self.mode = Mode::Bookmark(BookmarkOp::Delete),
            _ => {}
        }
        Outcome::None
    }
}
