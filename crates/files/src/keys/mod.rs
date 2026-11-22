//! The explorer's actions and their default keys (see `docs/usage-design.md`): the shared verbs
//! are single keys, everything else is under the leader menu. Each domain declares its own actions
//! next to the code that runs them (`panel/<domain>.rs`); this module only combines them.

use std::sync::LazyLock;
use ui::keys::{Commands, DefaultBinding, Group};

/// What the panel itself does, from each domain's own table (see `panel`).
pub static COMMANDS: LazyLock<Commands> = LazyLock::new(crate::panel::commands);

/// Names for the leader-menu prefixes, so the menu can label each submenu.
pub const GROUPS: &[Group] = &[
    Group { prefix: "leader v", name: "view" },
    Group { prefix: "leader c", name: "cd" },
    Group { prefix: "leader b", name: "bookmarks" },
    Group { prefix: "leader t", name: "tabs" },
];

/// The commands for a host that owns the keymap (the shared app shell): all of `COMMANDS` except
/// the movement, the palette and the help, which the shell registers itself.
pub fn hosted_commands() -> Commands {
    let shell_owned = |id: &str| id.starts_with("nav.") || matches!(id, "palette" | "help");
    Commands {
        actions: COMMANDS.actions.iter().filter(|a| !shell_owned(a.id)).cloned().collect(),
        default_bindings: COMMANDS
            .default_bindings
            .iter()
            .filter(|b| !shell_owned(b.id))
            .cloned()
            .collect::<Vec<DefaultBinding>>(),
    }
}

/// The ids of `COMMANDS`, to tell the panel's own actions from a host's.
pub fn is_own_action(id: &str) -> bool {
    COMMANDS.actions.iter().any(|a| a.id == id)
}

#[cfg(test)]
mod tests;
