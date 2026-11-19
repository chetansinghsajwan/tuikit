//! The table editor's actions and default keys (see `docs/usage-design.md`).

use ui::keys::{Action, Group, action, basic};

/// Every action of the editor with its default keys. One table feeds the keymap, the palette
/// and the help screen, so the three never disagree.
pub const ACTIONS: &[Action] = &[
    basic("nav.left", "left", "move to the cell on the left", &["h", "left"]),
    basic("nav.right", "right", "move to the cell on the right", &["l", "right"]),
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
    action(
        "sort.asc",
        "sort ascending",
        "sort the rows by this column (numbers as numbers)",
        &["s"],
    ),
    action("sort.desc", "sort descending", "sort the rows by this column, largest first", &["S"]),
    action(
        "view.header",
        "toggle header",
        "the first row is a header: it stays on top and is not sorted",
        &["leader t h"],
    ),
    action("find.start", "find", "jump to the next cell containing the text", &["/"]),
    action("find.next", "next match", "jump to the next match", &["n"]),
    action("find.prev", "previous match", "jump to the previous match", &["N"]),
    action("undo", "undo", "undo the last change", &["u"]),
    action("redo", "redo", "redo the change undone", &["ctrl-r"]),
    action("file.save", "save", "write the table to its file", &["ctrl-s", "leader w"]),
    basic(
        "quit",
        "quit",
        "close the editor (asks when there are unsaved changes)",
        &["q", "ctrl-c"],
    ),
];

/// Names for the key prefixes, shown when a prefix is pending.
pub const GROUPS: &[Group] =
    &[Group { prefix: "leader c", name: "column" }, Group { prefix: "leader t", name: "table" }];

/// Tests for the key table.
#[cfg(test)]
mod tests;
