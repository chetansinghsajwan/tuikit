//! The symbol explorer's actions and default keys (see `docs/usage-design.md`).

use ui::keys::{Action, Group};

/// Builds a basic action: movement and keys everyone knows. Kept out of the command palette.
macro_rules! b {
    ($id:literal, $name:literal, $desc:literal, [$($k:literal),*]) => {
        Action { id: $id, name: $name, desc: $desc, basic: true, keys: &[$($k),*] }
    };
}

/// Builds a non-basic action: one that shows in the command palette.
macro_rules! a {
    ($id:literal, $name:literal, $desc:literal, [$($k:literal),*]) => {
        Action { id: $id, name: $name, desc: $desc, basic: false, keys: &[$($k),*] }
    };
}

/// Every action of the explorer with its default keys. One table feeds the keymap, the palette
/// and the help screen, so the three never disagree.
pub const ACTIONS: &[Action] = &[
    b!("nav.up", "up", "move the cursor up", ["k", "up", "ctrl-p"]),
    b!("nav.down", "down", "move the cursor down", ["j", "down", "ctrl-n"]),
    b!("nav.up_step", "up a step", "move the cursor up several rows", ["ctrl-k"]),
    b!("nav.down_step", "down a step", "move the cursor down several rows", ["ctrl-j"]),
    b!("nav.page_up", "page up", "move a page up", ["pageup"]),
    b!("nav.page_down", "page down", "move a page down", ["pagedown"]),
    b!("nav.top", "top", "go to the first row", ["g", "home"]),
    b!("nav.bottom", "bottom", "go to the last row", ["G", "end"]),
    b!("tree.collapse", "collapse", "collapse the symbol, or go to its parent", ["h", "left"]),
    b!("tree.in", "expand", "expand the symbol", ["l", "right"]),
    b!(
        "item.open",
        "go to",
        "jump to the symbol in the editor (on one with members: also expand it)",
        ["enter"]
    ),
    a!("tree.fold_all", "fold all", "collapse every symbol with members", ["leader z c"]),
    a!("tree.unfold_all", "unfold all", "expand every symbol", ["leader z o"]),
    a!(
        "filter.start",
        "filter",
        "show only the symbols whose name contains the text",
        ["/", "leader f"]
    ),
    b!("palette", "commands", "search every action by name and run it", ["leader space"]),
    b!("help", "keys", "list every action with its keys", ["?"]),
    b!("quit", "back", "give the keyboard back to the editor", ["q", "esc", "ctrl-c"]),
];

/// Names for the key prefixes, shown when a prefix is pending.
pub const GROUPS: &[Group] = &[Group { prefix: "leader z", name: "fold" }];

/// The ids of `ACTIONS`, to tell the explorer's own actions from a host's.
pub fn is_own_action(id: &str) -> bool {
    ACTIONS.iter().any(|a| a.id == id)
}

/// Tests for the key table.
#[cfg(test)]
mod tests;
