//! The default keys of `tb`'s actions.

use ui::keys::DefaultBinding;

/// The default keys of [`ACTIONS`].
pub const DEFAULT_BINDINGS: &[DefaultBinding] = &[
    DefaultBinding { id: "nav.left", keys: &["h", "left"] },
    DefaultBinding { id: "nav.right", keys: &["l", "right"] },
    DefaultBinding { id: "nav.first_col", keys: &["0"] },
    DefaultBinding { id: "nav.last_col", keys: &["$"] },
    DefaultBinding { id: "cell.edit", keys: &["enter", "e"] },
    DefaultBinding { id: "cell.clear", keys: &["x"] },
    DefaultBinding { id: "cell.copy", keys: &["y"] },
    DefaultBinding { id: "cell.paste", keys: &["p"] },
    DefaultBinding { id: "row.add", keys: &["a"] },
    DefaultBinding { id: "row.add_above", keys: &["O"] },
    DefaultBinding { id: "row.delete", keys: &["d"] },
    DefaultBinding { id: "row.up", keys: &["K"] },
    DefaultBinding { id: "row.down", keys: &["J"] },
    DefaultBinding { id: "col.add", keys: &["leader c a"] },
    DefaultBinding { id: "col.delete", keys: &["leader c d"] },
    DefaultBinding { id: "col.left", keys: &["H", "leader c h"] },
    DefaultBinding { id: "col.right", keys: &["L", "leader c l"] },
    DefaultBinding { id: "sort.asc", keys: &["s"] },
    DefaultBinding { id: "sort.desc", keys: &["S"] },
    DefaultBinding { id: "view.header", keys: &["leader t h"] },
    DefaultBinding { id: "find.start", keys: &["/"] },
    DefaultBinding { id: "find.next", keys: &["n"] },
    DefaultBinding { id: "find.prev", keys: &["N"] },
    DefaultBinding { id: "undo", keys: &["u"] },
    DefaultBinding { id: "redo", keys: &["ctrl-r"] },
    DefaultBinding { id: "file.save", keys: &["ctrl-s", "leader w"] },
    DefaultBinding { id: "quit", keys: &["q", "ctrl-c"] },
];
