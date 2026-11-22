//! The git panel's actions and default keys (see `docs/usage-design.md`).

/// The default keys of the actions.
mod binds;
/// The actions and the names of the leader menus.
mod commands;

pub use commands::{ACTIONS, GROUPS};

/// The actions and their default keys together, as a keymap takes them.
pub fn commands() -> ui::keys::Commands {
    ui::keys::Commands::of(ACTIONS, binds::DEFAULT_BINDINGS)
}

/// The commands for a host that owns the keymap (the shared app shell): all of `commands()`
/// except the movement, the palette and the help, which the shell registers itself.
pub fn hosted_commands() -> ui::keys::Commands {
    let shell_owned = |id: &str| id.starts_with("nav.") || matches!(id, "palette" | "help");
    let all = commands();
    ui::keys::Commands {
        actions: all.actions.into_iter().filter(|a| !shell_owned(a.id)).collect(),
        default_bindings: all.default_bindings.into_iter().filter(|b| !shell_owned(b.id)).collect(),
    }
}

/// The ids of `ACTIONS`, to tell the panel's own actions from a host's.
pub fn is_own_action(id: &str) -> bool {
    ACTIONS.iter().any(|a| a.id == id)
}
