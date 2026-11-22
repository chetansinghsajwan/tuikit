//! `nu`'s actions and default keys (see `docs/usage-design.md`): both panes share the pane
//! switching and quit (movement, palette and help come from the shell); each pane adds its own.

use commands::{BLUETOOTH, COMMON, WIFI};
use ui::keys::Commands;

/// The default keys of both panes' actions.
mod binds;
/// The actions of both panes.
mod commands;

/// The commands of both panes.
pub fn common() -> Commands {
    Commands::of(COMMON, binds::COMMON)
}

/// The commands of the Wi-Fi (0) and Bluetooth (1) panes, one set each.
pub fn panes() -> Vec<Commands> {
    vec![Commands::of(WIFI, binds::WIFI), Commands::of(BLUETOOTH, binds::BLUETOOTH)]
}

#[cfg(test)]
mod tests;
