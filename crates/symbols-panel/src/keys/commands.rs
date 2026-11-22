//! The symbol explorer's actions and the names of its leader menus.

use ui::keys::{Action, Group};

/// Every action of the explorer with its default keys. One table feeds the keymap, the palette
/// and the help screen, so the three never disagree.
pub const ACTIONS: &[Action] = &[
    Action { id: "nav.up", name: "up", desc: "move the cursor up", basic: true },
    Action { id: "nav.down", name: "down", desc: "move the cursor down", basic: true },
    Action {
        id: "nav.up_step",
        name: "up a step",
        desc: "move the cursor up several rows",
        basic: true,
    },
    Action {
        id: "nav.down_step",
        name: "down a step",
        desc: "move the cursor down several rows",
        basic: true,
    },
    Action { id: "nav.page_up", name: "page up", desc: "move a page up", basic: true },
    Action { id: "nav.page_down", name: "page down", desc: "move a page down", basic: true },
    Action { id: "nav.top", name: "top", desc: "go to the first row", basic: true },
    Action { id: "nav.bottom", name: "bottom", desc: "go to the last row", basic: true },
    Action {
        id: "tree.collapse",
        name: "collapse",
        desc: "collapse the symbol, or go to its parent",
        basic: true,
    },
    Action { id: "tree.in", name: "expand", desc: "expand the symbol", basic: true },
    Action {
        id: "item.open",
        name: "go to",
        desc: "jump to the symbol in the editor (on one with members: also expand it)",
        basic: true,
    },
    Action {
        id: "tree.fold_all",
        name: "fold all",
        desc: "collapse every symbol with members",
        basic: false,
    },
    Action { id: "tree.unfold_all", name: "unfold all", desc: "expand every symbol", basic: false },
    Action {
        id: "filter.start",
        name: "filter",
        desc: "show only the symbols whose name contains the text",
        basic: false,
    },
    Action {
        id: "palette",
        name: "commands",
        desc: "search every action by name and run it",
        basic: true,
    },
    Action { id: "help", name: "keys", desc: "list every action with its keys", basic: true },
    Action { id: "quit", name: "back", desc: "give the keyboard back to the editor", basic: true },
];

/// Names for the key prefixes, shown when a prefix is pending.
pub const GROUPS: &[Group] = &[Group { prefix: "leader z", name: "fold" }];
