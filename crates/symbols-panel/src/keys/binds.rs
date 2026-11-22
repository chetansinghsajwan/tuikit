//! The default keys of the symbols panel's actions.

use ui::keys::DefaultBinding;

/// The default keys of [`ACTIONS`].
pub const DEFAULT_BINDINGS: &[DefaultBinding] = &[
    DefaultBinding { id: "nav.up", keys: &["k", "up", "ctrl-p"] },
    DefaultBinding { id: "nav.down", keys: &["j", "down", "ctrl-n"] },
    DefaultBinding { id: "nav.up_step", keys: &["ctrl-k"] },
    DefaultBinding { id: "nav.down_step", keys: &["ctrl-j"] },
    DefaultBinding { id: "nav.page_up", keys: &["pageup"] },
    DefaultBinding { id: "nav.page_down", keys: &["pagedown"] },
    DefaultBinding { id: "nav.top", keys: &["g", "home"] },
    DefaultBinding { id: "nav.bottom", keys: &["G", "end"] },
    DefaultBinding { id: "tree.collapse", keys: &["h", "left"] },
    DefaultBinding { id: "tree.in", keys: &["l", "right"] },
    DefaultBinding { id: "item.open", keys: &["enter"] },
    DefaultBinding { id: "tree.fold_all", keys: &["leader z c"] },
    DefaultBinding { id: "tree.unfold_all", keys: &["leader z o"] },
    DefaultBinding { id: "filter.start", keys: &["/", "leader f"] },
    DefaultBinding { id: "palette", keys: &["leader space"] },
    DefaultBinding { id: "help", keys: &["?"] },
    DefaultBinding { id: "quit", keys: &["q", "esc", "ctrl-c"] },
];
