//! The shell's own actions: movement, tabs, the palette and the help.

use crate::keys::{Action, Group};

/// The shared movement actions, registered when the screen is `Navigable`. The keys follow
/// `docs/usage-design.md`: `ctrl-d` and `ctrl-u` are left to the preview scroll.
pub(super) const NAV: &[Action] = &[
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
];

/// The tab actions of every screen with tabs.
pub(super) const TABS: &[Action] = &[
    Action { id: "tab.next", name: "next tab", desc: "switch to the next tab", basic: true },
    Action {
        id: "tab.prev",
        name: "previous tab",
        desc: "switch to the previous tab",
        basic: true,
    },
];

/// The extra tab actions of a screen whose tabs the user opens and closes.
pub(super) const DYNAMIC_TABS: &[Action] = &[
    Action {
        id: "tab.new",
        name: "new tab",
        desc: "open a new tab in the same place",
        basic: false,
    },
    Action {
        id: "tab.reopen",
        name: "reopen tab",
        desc: "bring back the tab closed last",
        basic: false,
    },
    Action {
        id: "tab.duplicate",
        name: "duplicate tab",
        desc: "open a copy of this tab next to it",
        basic: false,
    },
    Action {
        id: "tab.move_left",
        name: "move tab left",
        desc: "move this tab one place left",
        basic: false,
    },
    Action {
        id: "tab.move_right",
        name: "move tab right",
        desc: "move this tab one place right",
        basic: false,
    },
    Action { id: "tab.close", name: "close tab", desc: "close this tab", basic: false },
];

/// The actions of the shell itself, on every screen.
pub(super) const SHELL: &[Action] = &[
    Action {
        id: "palette",
        name: "commands",
        desc: "search every action by name and run it",
        basic: true,
    },
    Action { id: "help", name: "keys", desc: "list every action with its keys", basic: true },
];

/// The name of the leader menu behind `leader t`.
pub(super) const TAB_GROUP: Group = Group { prefix: "leader t", name: "tabs" };
