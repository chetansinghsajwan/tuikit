//! `ui`: shared TUI design framework. Tokens in `theme`, chrome in `pane`,
//! screen splitting in `layout`. Tools draw through these and never compute
//! their own borders, colors or gaps.

pub mod anim;
pub mod ansi;
pub mod clipboard;
pub mod config;
pub mod dim;
pub mod icons;
mod icons_data;
pub mod keys;
pub mod layout;
pub mod list;
pub mod multi;
pub mod pane;
pub mod popup;
pub mod prompt;
pub mod recent;
pub mod subject;
pub mod tabs;
pub mod theme;
pub mod time;
pub mod tree;
pub mod watch;

pub use pane::Pane;
pub use theme::{Theme, mix};
