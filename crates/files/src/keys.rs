//! The explorer's actions and their default keys (see `docs/usage-design.md`): the shared verbs
//! are single keys, everything else is under the leader menu. Each domain declares its own actions
//! next to the code that runs them (`panel/<domain>.rs`); this module only combines them.

use std::sync::LazyLock;
use ui::keys::{Action, Group};

/// What the panel itself does, from each domain's own table (see `panel`).
pub static ACTIONS: LazyLock<Vec<Action>> = LazyLock::new(crate::panel::actions);

/// Names for the leader-menu prefixes, so the menu can label each submenu.
pub const GROUPS: &[Group] = &[
    Group { prefix: "leader v", name: "view" },
    Group { prefix: "leader c", name: "cd" },
    Group { prefix: "leader b", name: "bookmarks" },
    Group { prefix: "leader t", name: "tabs" },
];

/// The actions for a host that owns the keymap (the shared app shell): all of `ACTIONS` except
/// the movement, the palette and the help, which the shell registers itself.
pub fn hosted() -> Vec<Action> {
    ACTIONS
        .iter()
        .filter(|a| !a.id.starts_with("nav.") && !matches!(a.id, "palette" | "help"))
        .cloned()
        .collect()
}

/// The ids of `ACTIONS`, to tell the panel's own actions from a host's.
pub fn owns(id: &str) -> bool {
    ACTIONS.iter().any(|a| a.id == id)
}

#[cfg(test)]
mod tests;
