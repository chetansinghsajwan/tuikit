//! The problems list's actions.

use ui::keys::Action;

/// Every action of the list. One table feeds the keymap, the palette and the help screen, so the
/// three never disagree.
pub const ACTIONS: &[Action] = &[
    Action { id: "nav.up", name: "up", desc: "move to the problem above", basic: true },
    Action { id: "nav.down", name: "down", desc: "move to the problem below", basic: true },
    Action { id: "nav.up_step", name: "up a step", desc: "move up several problems", basic: true },
    Action {
        id: "nav.down_step",
        name: "down a step",
        desc: "move down several problems",
        basic: true,
    },
    Action { id: "nav.page_up", name: "page up", desc: "move a page up", basic: true },
    Action { id: "nav.page_down", name: "page down", desc: "move a page down", basic: true },
    Action { id: "nav.top", name: "top", desc: "go to the first problem", basic: true },
    Action { id: "nav.bottom", name: "bottom", desc: "go to the last problem", basic: true },
    Action {
        id: "item.open",
        name: "go to",
        desc: "open the file at the problem in the editor",
        basic: true,
    },
    Action {
        id: "view.severity",
        name: "severity filter",
        desc: "cycle the list: everything, errors and warnings, errors only",
        basic: true,
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
