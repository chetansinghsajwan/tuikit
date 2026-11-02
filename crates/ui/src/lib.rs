//! `ui`: shared TUI design framework. Tokens in `theme`, chrome in `pane`,
//! screen splitting in `layout`. Tools draw through these and never compute
//! their own borders, colors or gaps.

pub mod ansi;
pub mod clipboard;
pub mod dim;
pub mod icons;
pub mod keys;
mod icons_data;
pub mod layout;
pub mod list;
pub mod popup;
pub mod prompt;
pub mod recent;
pub mod pane;
pub mod tabs;
pub mod theme;
pub mod watch;

pub use pane::Pane;
pub use theme::{Theme, mix};
