//! `ui`: shared TUI design framework. Tokens in `theme`, chrome in `pane`,
//! screen splitting in `layout`. Tools draw through these and never compute
//! their own borders, colors or gaps.

pub mod anim;
pub mod ansi;
pub mod app;
pub mod bigtext;
pub mod clipboard;
pub mod config;
pub mod dim;
pub mod footer;
pub mod graph;
pub mod icons;
pub mod job;
pub mod keys;
pub mod layout;
pub mod list;
pub mod multi;
pub mod pane;
pub mod pill;
pub mod popup;
pub mod prompt;
pub mod recent;
pub mod scroll;
pub mod subject;
pub mod symbol;
pub mod tabs;
pub mod theme;
pub mod time;
pub mod tree;
pub mod watch;

pub use pane::Pane;
pub use theme::{Theme, mix};
