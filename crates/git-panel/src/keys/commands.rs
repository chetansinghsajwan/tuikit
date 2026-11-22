//! The git panel's actions and the names of its leader menus.

use ui::keys::{Action, Group};

/// Every action of the panel with its default keys. One table feeds the keymap, the palette
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
        id: "preview.down",
        name: "scroll diff down",
        desc: "scroll the diff down",
        basic: true,
    },
    Action { id: "preview.up", name: "scroll diff up", desc: "scroll the diff up", basic: true },
    Action {
        id: "preview.next_hunk",
        name: "next hunk",
        desc: "scroll the diff to the next hunk",
        basic: true,
    },
    Action {
        id: "preview.prev_hunk",
        name: "previous hunk",
        desc: "scroll the diff to the previous hunk",
        basic: true,
    },
    Action {
        id: "item.open",
        name: "open",
        desc: "open the file in your editor (on a folder: fold or unfold it)",
        basic: true,
    },
    Action {
        id: "change.stage",
        name: "stage or unstage",
        desc: "stage what is not staged yet in the file or folder under the cursor, or unstage it when it is all staged",
        basic: false,
    },
    Action {
        id: "change.unstage",
        name: "unstage",
        desc: "unstage the file or folder under the cursor",
        basic: false,
    },
    Action {
        id: "change.toggle_all",
        name: "stage or unstage all",
        desc: "stage every change, or unstage everything when all of it is staged already",
        basic: false,
    },
    Action {
        id: "change.discard",
        name: "discard",
        desc: "throw away the work-tree changes of the file (asks first)",
        basic: false,
    },
    Action { id: "commit.new", name: "commit", desc: "commit what is staged", basic: false },
    Action {
        id: "commit.amend",
        name: "amend",
        desc: "add what is staged to the last commit, keeping its message",
        basic: false,
    },
    Action {
        id: "view.tree",
        name: "tree or list",
        desc: "show the changed files as a tree of folders or as a flat list",
        basic: false,
    },
    Action { id: "view.refresh", name: "refresh", desc: "reread the repository", basic: false },
    Action {
        id: "palette",
        name: "commands",
        desc: "search every action by name and run it",
        basic: true,
    },
    Action { id: "help", name: "keys", desc: "list every action with its keys", basic: true },
    Action { id: "quit", name: "quit", desc: "close the panel", basic: true },
];

/// Names for the key prefixes (none here: the panel has no multi-key groups).
pub const GROUPS: &[Group] = &[];
