//! The explorer's actions and their default keys (see `docs/usage-design.md`):
//! the shared verbs are single keys, everything else is under the leader menu.

use ui::keys::{Action, Group};

macro_rules! a {
    ($id:literal, $name:literal, $desc:literal, [$($k:literal),*]) => {
        Action { id: $id, name: $name, desc: $desc, keys: &[$($k),*] }
    };
}

/// What the panel itself does.
pub const ACTIONS: &[Action] = &[
    a!("nav.up", "up", "move the cursor up", ["k", "up", "ctrl-p"]),
    a!("nav.down", "down", "move the cursor down", ["j", "down", "ctrl-n"]),
    a!("nav.page_up", "page up", "move a page up", ["pageup"]),
    a!("nav.page_down", "page down", "move a page down", ["pagedown"]),
    a!("nav.top", "top", "go to the first row", ["g", "home"]),
    a!("nav.bottom", "bottom", "go to the last row", ["G", "end"]),
    a!("tree.out", "collapse", "collapse the folder, or go to the parent folder's row", ["h", "left"]),
    a!("item.open", "open", "open the file, or expand the folder", ["enter", "l", "right"]),
    a!("item.mark", "mark", "mark or unmark the row for a multi-row action", ["tab"]),
    a!("item.new", "new", "create a file (a trailing / makes a folder)", ["a"]),
    a!("item.rename", "rename", "rename the row", ["r"]),
    a!("item.delete", "delete", "move the marked rows to the trash (asks first)", ["d"]),
    a!("item.delete_forever", "delete forever", "delete the marked rows permanently (asks first)", ["D"]),
    a!("item.yank", "copy", "copy the marked rows", ["y"]),
    a!("item.cut", "cut", "cut the marked rows", ["x"]),
    a!("item.paste", "paste", "paste into the folder under the cursor", ["p"]),
    a!("item.open_with", "open with", "open the row with a command you type", ["leader o"]),
    a!("item.bulk_rename", "bulk rename", "rename the marked rows together in your editor", ["leader B"]),
    a!("search.files", "find", "find files by name (the picker)", ["/", "leader f"]),
    a!("search.grep", "grep", "search file contents (the picker)", ["leader s"]),
    a!("view.hidden", "hidden files", "show or hide dot files", [".", "leader v h"]),
    a!("view.columns", "columns", "cycle the columns: permissions, size and time / size and time / permissions and time / off", ["leader v c"]),
    a!("view.refresh", "refresh", "reread the folders and the git status", ["R", "leader v r"]),
    a!("root.into", "cd into", "make the folder under the cursor the root", ["leader c l"]),
    a!("root.up", "cd up", "make the parent folder the root", ["leader c h"]),
    a!("bookmark.set", "save bookmark", "save the folder under the cursor under a key you type", ["leader b s"]),
    a!("bookmark.jump", "go to bookmark", "make a bookmarked folder the root", ["leader b j"]),
    a!("bookmark.delete", "delete bookmark", "delete a bookmark", ["leader b d"]),
    a!("palette", "commands", "search every action by name and run it", ["leader space"]),
    a!("help", "keys", "list every action with its keys", ["?"]),
    a!("back", "back", "clear the marks, else close", ["esc"]),
    a!("quit", "quit", "close the explorer", ["q", "ctrl-c"]),
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
mod tests {
    use super::*;
    use ui::keys::Keymap;

    #[test]
    fn every_action_is_described_and_no_two_share_a_key() {
        let mut ids = std::collections::HashSet::new();
        for a in ACTIONS {
            assert!(!a.name.is_empty() && !a.desc.is_empty() && !a.keys.is_empty(), "{}", a.id);
            assert!(ids.insert(a.id), "duplicate id {}", a.id);
        }
        // Building the keymap parses every key; none may be dropped or shared.
        let map = Keymap::new("space", ACTIONS, GROUPS);
        for a in ACTIONS {
            assert!(!map.keys_of(a.id).is_empty(), "{} has no parsable key", a.id);
        }
        let mut shown = std::collections::HashSet::new();
        for a in ACTIONS {
            for k in a.keys {
                assert!(shown.insert(*k), "key {k} bound twice");
            }
        }
    }
}
