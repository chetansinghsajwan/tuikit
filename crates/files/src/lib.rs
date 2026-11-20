//! `files`: the shared file browser panel. A library (tree model, key handling,
//! rendering through `ui`, git state from `git-core`, file operations) that
//! editors embed as a sidebar; a standalone binary can wrap it later.

/// Saved key-to-folder bookmarks.
pub mod bookmarks;
/// The panel's actions and their default keys.
pub mod keys;
/// File properties for the metadata panel and the size, time and permission cells of columns.
pub mod meta;
/// File operations behind the keys (create, rename, copy, move, trash, bulk rename).
pub mod ops;
/// The panel itself: tree state, key handling and drawing.
mod panel;

pub use panel::{Linemode, Outcome, Panel, PanelOptions, Search};
