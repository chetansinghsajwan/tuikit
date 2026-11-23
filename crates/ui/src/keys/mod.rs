//! The action registry behind every tool's keys (see `docs/usage-design.md`).
//!
//! A tool declares its actions (id, name, description) and, apart from them, the default keys that run them, and feeds key
//! events to a [`Keymap`]; the keymap resolves single keys and sequences such as
//! `leader v c`, and describes what can follow a started sequence ([`Menu`], the
//! which-key popup drawn by [`render_menu`]). Names and descriptions are what a
//! command palette and `?` help list.
//!
//! Key names: a character (`j`, `G`, `.`), or `space`, `enter`, `esc`, `tab`,
//! `backtab`, `backspace`, `up`, `down`, `left`, `right`, `home`, `end`, `pageup`,
//! `pagedown`, with `ctrl-` or `alt-` in front. `leader` stands for the configured
//! leader key. A binding is one key or several separated by spaces.

mod chord;
mod commands;
mod help;
mod keymap;
mod menu;
mod palette;

pub use chord::{Chord, pressed};
pub use commands::{Action, Commands, DefaultBinding, Group};
pub use help::Help;
pub use keymap::{Feed, Keymap};
pub use menu::{MENU_TITLE, MENU_WIDTH, Menu, MenuEntry, menu_height, render_menu};
pub use palette::{Row, choose};

#[cfg(test)]
mod tests;
