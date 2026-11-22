//! The default keys of the file panel's actions.

use ui::keys::DefaultBinding;

/// The default keys of [`NAV`](super::commands::NAV).
pub(super) const NAV: &[DefaultBinding] = &[
    DefaultBinding { id: "nav.up", keys: &["k", "up", "ctrl-p"] },
    DefaultBinding { id: "nav.down", keys: &["j", "down", "ctrl-n"] },
    DefaultBinding { id: "nav.up_step", keys: &["ctrl-k"] },
    DefaultBinding { id: "nav.down_step", keys: &["ctrl-j"] },
    DefaultBinding { id: "nav.page_up", keys: &["pageup"] },
    DefaultBinding { id: "nav.page_down", keys: &["pagedown"] },
    DefaultBinding { id: "nav.top", keys: &["g", "home"] },
    DefaultBinding { id: "nav.bottom", keys: &["G", "end"] },
    DefaultBinding { id: "tree.collapse", keys: &["h", "left"] },
];

/// The default keys of [`ITEM`](super::commands::ITEM).
pub(super) const ITEM: &[DefaultBinding] = &[
    DefaultBinding { id: "item.open", keys: &["enter", "l", "right"] },
    DefaultBinding { id: "item.mark", keys: &["tab"] },
    DefaultBinding { id: "item.create", keys: &["a", "leader n"] },
    DefaultBinding { id: "item.rename", keys: &["r", "leader r"] },
    DefaultBinding { id: "item.delete", keys: &["d", "leader d"] },
    DefaultBinding { id: "item.delete_forever", keys: &["D"] },
    DefaultBinding { id: "item.copy", keys: &["y", "leader y"] },
    DefaultBinding { id: "item.cut", keys: &["x", "leader x"] },
    DefaultBinding { id: "item.paste", keys: &["p", "leader p"] },
    DefaultBinding { id: "item.open_with", keys: &["leader o"] },
    DefaultBinding { id: "item.bulk_rename", keys: &["leader B"] },
];

/// The default keys of [`SEARCH`](super::commands::SEARCH).
pub(super) const SEARCH: &[DefaultBinding] = &[
    DefaultBinding { id: "search.here", keys: &["/"] },
    DefaultBinding { id: "search.files", keys: &["leader f"] },
    DefaultBinding { id: "search.grep", keys: &["leader s"] },
];

/// The default keys of [`VIEW`](super::commands::VIEW).
pub(super) const VIEW: &[DefaultBinding] = &[
    DefaultBinding { id: "view.hidden", keys: &[".", "leader v h"] },
    DefaultBinding { id: "view.ignored", keys: &["I", "leader v I"] },
    DefaultBinding { id: "view.columns", keys: &["leader v c"] },
    DefaultBinding { id: "view.refresh", keys: &["R", "leader v r"] },
    DefaultBinding { id: "view.tables_as_text", keys: &["leader v t"] },
];

/// The default keys of [`ROOT`](super::commands::ROOT).
pub(super) const ROOT: &[DefaultBinding] = &[
    DefaultBinding { id: "root.into", keys: &["leader c l"] },
    DefaultBinding { id: "root.focus", keys: &["leader c f"] },
    DefaultBinding { id: "root.up", keys: &["leader c h"] },
];

/// The default keys of [`BOOKMARK`](super::commands::BOOKMARK).
pub(super) const BOOKMARK: &[DefaultBinding] = &[
    DefaultBinding { id: "bookmark.set", keys: &["leader b s"] },
    DefaultBinding { id: "bookmark.jump", keys: &["leader b j"] },
    DefaultBinding { id: "bookmark.delete", keys: &["leader b d"] },
];

/// The default keys of [`GENERAL`](super::GENERAL).
pub(super) const GENERAL: &[DefaultBinding] = &[
    DefaultBinding { id: "palette", keys: &["leader space"] },
    DefaultBinding { id: "help", keys: &["?"] },
    DefaultBinding { id: "back", keys: &["esc"] },
    DefaultBinding { id: "quit", keys: &["q", "ctrl-c", "leader q"] },
];
