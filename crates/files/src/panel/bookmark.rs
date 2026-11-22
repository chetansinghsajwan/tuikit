//! Bookmarks: folders saved under a key.

use super::*;

impl Panel {
    /// Runs one of this domain's actions by id.
    pub(super) fn run_bookmark_action(&mut self, id: &str) -> Outcome {
        match id {
            "bookmark.set" => self.mode = Mode::Bookmark(BookmarkOp::Set),
            "bookmark.jump" => self.mode = Mode::Bookmark(BookmarkOp::Jump),
            "bookmark.delete" => self.mode = Mode::Bookmark(BookmarkOp::Delete),
            _ => {}
        }
        Outcome::None
    }
}
