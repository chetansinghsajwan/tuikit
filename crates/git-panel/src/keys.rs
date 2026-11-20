//! The git panel's actions and default keys (see `docs/usage-design.md`).

use ui::keys::{Action, Group, action, basic};

/// Every action of the panel with its default keys. One table feeds the keymap, the palette
/// and the help screen, so the three never disagree.
pub const ACTIONS: &[Action] = &[
    basic("nav.up", "up", "move the cursor up", &["k", "up", "ctrl-p"]),
    basic("nav.down", "down", "move the cursor down", &["j", "down", "ctrl-n"]),
    basic("nav.up_step", "up a step", "move the cursor up several rows", &["ctrl-k"]),
    basic("nav.down_step", "down a step", "move the cursor down several rows", &["ctrl-j"]),
    basic("nav.page_up", "page up", "move a page up", &["pageup"]),
    basic("nav.page_down", "page down", "move a page down", &["pagedown"]),
    basic("nav.top", "top", "go to the first row", &["g", "home"]),
    basic("nav.bottom", "bottom", "go to the last row", &["G", "end"]),
    basic("preview.down", "scroll diff down", "scroll the diff down", &["ctrl-d"]),
    basic("preview.up", "scroll diff up", "scroll the diff up", &["ctrl-u"]),
    basic("preview.next_hunk", "next hunk", "scroll the diff to the next hunk", &["}"]),
    basic("preview.prev_hunk", "previous hunk", "scroll the diff to the previous hunk", &["{"]),
    basic(
        "item.open",
        "open",
        "open the file in your editor (on a folder: fold or unfold it)",
        &["enter", "l", "right"],
    ),
    action(
        "change.stage",
        "stage or unstage",
        "stage what is not staged yet in the file or folder under the cursor, or unstage it when it is all staged",
        &["s", "leader s"],
    ),
    action(
        "change.unstage",
        "unstage",
        "unstage the file or folder under the cursor",
        &["u", "leader u"],
    ),
    action(
        "change.toggle_all",
        "stage or unstage all",
        "stage every change, or unstage everything when all of it is staged already",
        &["a", "leader a"],
    ),
    action(
        "change.discard",
        "discard",
        "throw away the work-tree changes of the file (asks first)",
        &["d", "leader d"],
    ),
    action("commit.new", "commit", "commit what is staged", &["c", "leader c"]),
    action(
        "commit.amend",
        "amend",
        "add what is staged to the last commit, keeping its message",
        &["C", "leader C"],
    ),
    action(
        "view.tree",
        "tree or list",
        "show the changed files as a tree of folders or as a flat list",
        &["t", "leader t"],
    ),
    action("view.refresh", "refresh", "reread the repository", &["leader R"]),
    basic("palette", "commands", "search every action by name and run it", &["leader space"]),
    basic("help", "keys", "list every action with its keys", &["?"]),
    basic("quit", "quit", "close the panel", &["q", "esc", "ctrl-c", "leader q"]),
];

/// Names for the key prefixes (none here: the panel has no multi-key groups).
pub const GROUPS: &[Group] = &[];

/// The actions for a host that owns the keymap (the shared app shell): all of `ACTIONS` except
/// the movement, the palette and the help, which the shell registers itself.
pub fn hosted_actions() -> Vec<Action> {
    ACTIONS
        .iter()
        .filter(|a| !a.id.starts_with("nav.") && !matches!(a.id, "palette" | "help"))
        .cloned()
        .collect()
}

/// The ids of `ACTIONS`, to tell the panel's own actions from a host's.
pub fn is_own_action(id: &str) -> bool {
    ACTIONS.iter().any(|a| a.id == id)
}
