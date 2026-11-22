//! The table editor's actions and the names of its leader menus.

use ui::keys::{Action, Group};

/// Every action of the editor with its default keys. One table feeds the keymap, the palette
/// and the help screen, so the three never disagree.
pub const ACTIONS: &[Action] = &[
    Action { id: "nav.left", name: "left", desc: "move to the cell on the left", basic: true },
    Action { id: "nav.right", name: "right", desc: "move to the cell on the right", basic: true },
    Action {
        id: "nav.first_col",
        name: "first column",
        desc: "go to the first column",
        basic: true,
    },
    Action { id: "nav.last_col", name: "last column", desc: "go to the last column", basic: true },
    Action { id: "cell.edit", name: "edit cell", desc: "edit the text of the cell", basic: false },
    Action { id: "cell.clear", name: "clear cell", desc: "empty the cell", basic: false },
    Action { id: "cell.copy", name: "copy cell", desc: "copy the text of the cell", basic: false },
    Action {
        id: "cell.paste",
        name: "paste",
        desc: "put the last copied text in the cell",
        basic: false,
    },
    Action { id: "row.add", name: "add row", desc: "add an empty row below", basic: false },
    Action {
        id: "row.add_above",
        name: "add row above",
        desc: "add an empty row above",
        basic: false,
    },
    Action {
        id: "row.delete",
        name: "delete row",
        desc: "delete the row (asks, `d` again confirms)",
        basic: false,
    },
    Action { id: "row.up", name: "move row up", desc: "move the row one up", basic: false },
    Action { id: "row.down", name: "move row down", desc: "move the row one down", basic: false },
    Action {
        id: "col.add",
        name: "add column",
        desc: "add an empty column after this one",
        basic: false,
    },
    Action {
        id: "col.delete",
        name: "delete column",
        desc: "delete the column (asks)",
        basic: false,
    },
    Action {
        id: "col.left",
        name: "move column left",
        desc: "move the column one left",
        basic: false,
    },
    Action {
        id: "col.right",
        name: "move column right",
        desc: "move the column one right",
        basic: false,
    },
    Action {
        id: "sort.asc",
        name: "sort ascending",
        desc: "sort the rows by this column (numbers as numbers)",
        basic: false,
    },
    Action {
        id: "sort.desc",
        name: "sort descending",
        desc: "sort the rows by this column, largest first",
        basic: false,
    },
    Action {
        id: "view.header",
        name: "toggle header",
        desc: "the first row is a header: it stays on top and is not sorted",
        basic: false,
    },
    Action {
        id: "find.start",
        name: "find",
        desc: "jump to the next cell containing the text",
        basic: false,
    },
    Action { id: "find.next", name: "next match", desc: "jump to the next match", basic: false },
    Action {
        id: "find.prev",
        name: "previous match",
        desc: "jump to the previous match",
        basic: false,
    },
    Action { id: "undo", name: "undo", desc: "undo the last change", basic: false },
    Action { id: "redo", name: "redo", desc: "redo the change undone", basic: false },
    Action { id: "file.save", name: "save", desc: "write the table to its file", basic: false },
    Action {
        id: "quit",
        name: "quit",
        desc: "close the editor (asks when there are unsaved changes)",
        basic: true,
    },
];

/// Names for the key prefixes, shown when a prefix is pending.
pub const GROUPS: &[Group] =
    &[Group { prefix: "leader c", name: "column" }, Group { prefix: "leader t", name: "table" }];
