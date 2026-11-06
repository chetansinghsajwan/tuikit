//! The explorer's actions and their default keys (see `docs/usage-design.md`):
//! the shared verbs are single keys, everything else is under the leader menu.

use ui::keys::{Action, Group};

macro_rules! a {
    ($id:literal, $name:literal, $desc:literal, [$($k:literal),*]) => {
        Action { id: $id, name: $name, desc: $desc, basic: false, keys: &[$($k),*] }
    };
}

/// A basic action: movement and keys everyone knows. Kept out of the command palette.
macro_rules! b {
    ($id:literal, $name:literal, $desc:literal, [$($k:literal),*]) => {
        Action { id: $id, name: $name, desc: $desc, basic: true, keys: &[$($k),*] }
    };
}

/// What the panel itself does.
pub const ACTIONS: &[Action] = &[
    b!("nav.up", "up", "move the cursor up", ["k", "up", "ctrl-p"]),
    b!("nav.down", "down", "move the cursor down", ["j", "down", "ctrl-n"]),
    b!("nav.up5", "up 5", "move the cursor up five rows", ["ctrl-k"]),
    b!("nav.down5", "down 5", "move the cursor down five rows", ["ctrl-j"]),
    b!("nav.page_up", "page up", "move a page up", ["pageup"]),
    b!("nav.page_down", "page down", "move a page down", ["pagedown"]),
    b!("nav.top", "top", "go to the first row", ["g", "home"]),
    b!("nav.bottom", "bottom", "go to the last row", ["G", "end"]),
    b!("tree.out", "collapse", "collapse the folder, or go to the parent folder's row", ["h", "left"]),
    b!("item.open", "open", "open the file, or expand the folder", ["enter", "l", "right"]),
    b!("item.mark", "mark", "mark or unmark the row for a multi-row action", ["tab"]),
    a!("item.new", "new", "create a file (a trailing / makes a folder)", ["a", "leader n"]),
    a!("item.rename", "rename", "rename the row", ["r", "leader r"]),
    a!("item.delete", "delete", "move the marked rows to the trash (asks first)", ["d", "leader d"]),
    a!("item.delete_forever", "delete forever", "delete the marked rows permanently (asks first)", ["D"]),
    a!("item.yank", "copy", "copy the marked rows", ["y", "leader y"]),
    a!("item.cut", "cut", "cut the marked rows", ["x", "leader x"]),
    a!("item.paste", "paste", "paste into the folder under the cursor", ["p", "leader p"]),
    a!("item.open_with", "open with", "open the row with a command you type", ["leader o"]),
    a!("item.bulk_rename", "bulk rename", "rename the marked rows together in your editor", ["leader B"]),
    a!("search.here", "find here", "find names in the folder under the cursor only, not in its subfolders (the picker)", ["/"]),
    a!("search.files", "find everywhere", "find files by name in the root and every subfolder (the picker)", ["leader f"]),
    a!("search.grep", "grep", "search file contents (the picker)", ["leader s"]),
    a!("view.hidden", "hidden files", "show or hide dot files", [".", "leader v h"]),
    a!("view.ignored", "ignored files", "show or hide the entries .gitignore and the other ignore files hide", ["I", "leader v I"]),
    a!("view.columns", "columns", "cycle the columns: permissions, size and time / size and time / permissions and time / off", ["leader v c"]),
    a!("view.refresh", "refresh", "reread the folders and the git status", ["R", "leader v r"]),
    a!("view.raw", "tables as text", "show CSV, TSV and spreadsheets in the preview as text instead of tables (and back)", ["leader v t"]),
    a!("root.into", "cd into", "make the folder under the cursor the root", ["leader c l"]),
    a!("root.focus", "focus folder", "make the folder the cursor row sits in the root, so only its own entries show", ["leader c f"]),
    a!("root.up", "cd up", "make the parent folder the root", ["leader c h"]),
    a!("bookmark.set", "save bookmark", "save the folder under the cursor under a key you type", ["leader b s"]),
    a!("bookmark.jump", "go to bookmark", "make a bookmarked folder the root", ["leader b j"]),
    a!("bookmark.delete", "delete bookmark", "delete a bookmark", ["leader b d"]),
    b!("palette", "commands", "search every action by name and run it", ["leader space"]),
    a!("help", "keys", "list every action with its keys", ["?"]),
    b!("back", "back", "clear the marks, else close", ["esc"]),
    b!("quit", "quit", "close the explorer", ["q", "ctrl-c", "leader q"]),
];

pub const GROUPS: &[Group] = &[
    Group { prefix: "leader v", name: "view" },
    Group { prefix: "leader c", name: "cd" },
    Group { prefix: "leader b", name: "bookmarks" },
    Group { prefix: "leader t", name: "tabs" },
];

/// The ids of `ACTIONS`, to tell the panel's own actions from a host's.
pub fn owns(id: &str) -> bool {
    ACTIONS.iter().any(|a| a.id == id)
}

#[cfg(test)]
mod tests;
