//! The monitor's actions and default keys (see `docs/usage-design.md`).

/// The default keys of the actions.
mod binds;
/// The actions and the names of the leader menus.
mod commands;

pub use commands::{ACTIONS, GROUPS};

/// The actions and their default keys together, as a keymap takes them.
pub fn commands() -> ui::keys::Commands {
    ui::keys::Commands::of(ACTIONS, binds::DEFAULT_BINDINGS)
}

/// Tests for the key table.
#[cfg(test)]
mod tests;
