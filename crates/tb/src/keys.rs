//! The table editor's actions and default keys (see `docs/usage-design.md`).

use ui::keys::{Action, Group, action, basic};

pub const ACTIONS: &[Action] = &[
    basic("nav.up", "up", "move to the cell above", &["k", "up", "ctrl-p"]),
    basic("nav.down", "down", "move to the cell below", &["j", "down", "ctrl-n"]),
    basic("nav.left", "left", "move to the cell on the left", &["h", "left"]),
    basic("nav.right", "right", "move to the cell on the right", &["l", "right"]),
    basic("nav.up5", "up 5", "move five rows up", &["ctrl-k"]),
    basic("nav.down5", "down 5", "move five rows down", &["ctrl-j"]),
    basic("nav.page_up", "page up", "move half a page up", &["ctrl-u", "pageup"]),
    basic("nav.page_down", "page down", "move half a page down", &["ctrl-d", "pagedown"]),
    basic("nav.top", "top", "go to the first row", &["g", "home"]),
    basic("nav.bottom", "bottom", "go to the last row", &["G", "end"]),
    basic("nav.first_col", "first column", "go to the first column", &["0"]),
    basic("nav.last_col", "last column", "go to the last column", &["$"]),
    action("cell.edit", "edit cell", "edit the text of the cell", &["enter", "e"]),
    action("cell.clear", "clear cell", "empty the cell", &["x"]),
    action("cell.copy", "copy cell", "copy the text of the cell", &["y"]),
    action("cell.paste", "paste", "put the last copied text in the cell", &["p"]),
    action("row.add", "add row", "add an empty row below", &["a"]),
    action("row.add_above", "add row above", "add an empty row above", &["O"]),
    action("row.delete", "delete row", "delete the row (asks, `d` again confirms)", &["d"]),
    action("row.up", "move row up", "move the row one up", &["K"]),
    action("row.down", "move row down", "move the row one down", &["J"]),
    action("col.add", "add column", "add an empty column after this one", &["leader c a"]),
    action("col.delete", "delete column", "delete the column (asks)", &["leader c d"]),
    action("col.left", "move column left", "move the column one left", &["H", "leader c h"]),
    action("col.right", "move column right", "move the column one right", &["L", "leader c l"]),
    action("sort.asc", "sort ascending", "sort the rows by this column (numbers as numbers)", &["s"]),
    action("sort.desc", "sort descending", "sort the rows by this column, largest first", &["S"]),
    action("view.header", "toggle header", "the first row is a header: it stays on top and is not sorted", &["leader t h"]),
    action("find.start", "find", "jump to the next cell containing the text", &["/"]),
    action("find.next", "next match", "jump to the next match", &["n"]),
    action("find.prev", "previous match", "jump to the previous match", &["N"]),
    action("undo", "undo", "undo the last change", &["u"]),
    action("redo", "redo", "redo the change undone", &["ctrl-r"]),
    action("file.save", "save", "write the table to its file", &["ctrl-s", "leader w"]),
    basic("palette", "commands", "search every action by name and run it", &["leader space"]),
    basic("help", "keys", "list every action with its keys", &["?"]),
    basic("quit", "quit", "close the editor (asks when there are unsaved changes)", &["q", "ctrl-c"]),
];

pub const GROUPS: &[Group] = &[Group { prefix: "leader c", name: "column" }, Group { prefix: "leader t", name: "table" }];

#[cfg(test)]
mod tests {
    use super::*;
    use ui::keys::Keymap;

    #[test]
    fn every_action_is_described_parsable_and_no_key_is_shared() {
        let map = Keymap::new("space", ACTIONS, GROUPS);
        let mut keys = std::collections::HashSet::new();
        for a in ACTIONS {
            assert!(!a.name.is_empty() && !a.desc.is_empty() && !a.keys.is_empty(), "{}", a.id);
            assert!(!map.keys_of(a.id).is_empty(), "{} has no parsable key", a.id);
            for k in a.keys {
                assert!(keys.insert(*k), "key {k} bound twice");
            }
        }
    }
}
