//! The default keys of the git panel's actions.

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
    DefaultBinding { id: "preview.down", keys: &["ctrl-d"] },
    DefaultBinding { id: "preview.up", keys: &["ctrl-u"] },
    DefaultBinding { id: "preview.next_hunk", keys: &["}"] },
    DefaultBinding { id: "preview.prev_hunk", keys: &["{"] },
    DefaultBinding { id: "item.open", keys: &["enter", "l", "right"] },
    DefaultBinding { id: "change.stage", keys: &["s", "leader s"] },
    DefaultBinding { id: "change.unstage", keys: &["u", "leader u"] },
    DefaultBinding { id: "change.toggle_all", keys: &["a", "leader a"] },
    DefaultBinding { id: "change.discard", keys: &["d", "leader d"] },
    DefaultBinding { id: "commit.new", keys: &["c", "leader c"] },
    DefaultBinding { id: "commit.amend", keys: &["C", "leader C"] },
    DefaultBinding { id: "view.tree", keys: &["t", "leader t"] },
    DefaultBinding { id: "view.refresh", keys: &["leader R"] },
    DefaultBinding { id: "palette", keys: &["leader space"] },
    DefaultBinding { id: "help", keys: &["?"] },
    DefaultBinding { id: "quit", keys: &["q", "esc", "ctrl-c", "leader q"] },
];
