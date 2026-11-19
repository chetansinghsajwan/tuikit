//! The git panel's actions and default keys (see `docs/usage-design.md`).

use ui::keys::{Action, Group};

/// Builds a non-basic action: one that shows in the command palette.
macro_rules! a {
    ($id:literal, $name:literal, $desc:literal, [$($k:literal),*]) => {
        Action { id: $id, name: $name, desc: $desc, basic: false, keys: &[$($k),*] }
    };
}

/// Basic: movement and keys everyone knows. Kept out of the command palette.
macro_rules! b {
    ($id:literal, $name:literal, $desc:literal, [$($k:literal),*]) => {
        Action { id: $id, name: $name, desc: $desc, basic: true, keys: &[$($k),*] }
    };
}

/// Every action of the panel with its default keys. One table feeds the keymap, the palette
/// and the help screen, so the three never disagree.
pub const ACTIONS: &[Action] = &[
    b!("nav.up", "up", "move the cursor up", ["k", "up", "ctrl-p"]),
    b!("nav.down", "down", "move the cursor down", ["j", "down", "ctrl-n"]),
    b!("nav.up5", "up 5", "move the cursor up five rows", ["ctrl-k"]),
    b!("nav.down5", "down 5", "move the cursor down five rows", ["ctrl-j"]),
    b!("nav.page_up", "page up", "move a page up", ["pageup"]),
    b!("nav.page_down", "page down", "move a page down", ["pagedown"]),
    b!("nav.top", "top", "go to the first row", ["g", "home"]),
    b!("nav.bottom", "bottom", "go to the last row", ["G", "end"]),
    b!("preview.down", "scroll diff down", "scroll the diff down", ["ctrl-d"]),
    b!("preview.up", "scroll diff up", "scroll the diff up", ["ctrl-u"]),
    b!("preview.next_hunk", "next hunk", "scroll the diff to the next hunk", ["}"]),
    b!("preview.prev_hunk", "previous hunk", "scroll the diff to the previous hunk", ["{"]),
    b!(
        "item.open",
        "open",
        "open the file in your editor (on a folder: fold or unfold it)",
        ["enter", "l", "right"]
    ),
    a!(
        "change.stage",
        "stage or unstage",
        "stage what is not staged yet in the file or folder under the cursor, or unstage it when it is all staged",
        ["s", "leader s"]
    ),
    a!(
        "change.unstage",
        "unstage",
        "unstage the file or folder under the cursor",
        ["u", "leader u"]
    ),
    a!(
        "change.toggle_all",
        "stage or unstage all",
        "stage every change, or unstage everything when all of it is staged already",
        ["a", "leader a"]
    ),
    a!(
        "change.discard",
        "discard",
        "throw away the work-tree changes of the file (asks first)",
        ["d", "leader d"]
    ),
    a!("commit.new", "commit", "commit what is staged", ["c", "leader c"]),
    a!(
        "commit.amend",
        "amend",
        "add what is staged to the last commit, keeping its message",
        ["C", "leader C"]
    ),
    a!(
        "view.tree",
        "tree or list",
        "show the changed files as a tree of folders or as a flat list",
        ["t", "leader t"]
    ),
    a!("view.refresh", "refresh", "reread the repository", ["leader R"]),
    b!("palette", "commands", "search every action by name and run it", ["leader space"]),
    b!("help", "keys", "list every action with its keys", ["?"]),
    b!("quit", "quit", "close the panel", ["q", "esc", "ctrl-c", "leader q"]),
];

/// Names for the key prefixes (none here: the panel has no multi-key groups).
pub const GROUPS: &[Group] = &[];

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
