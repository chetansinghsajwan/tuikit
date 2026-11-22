//! The default keys of the screen's actions.

use ui::keys::DefaultBinding;

/// The default keys of [`ACTIONS`].
pub const DEFAULT_BINDINGS: &[DefaultBinding] = &[
    DefaultBinding { id: "quit", keys: &["ctrl-q"] },
    DefaultBinding { id: "layer.next", keys: &["ctrl-n"] },
    DefaultBinding { id: "layer.prev", keys: &["ctrl-p"] },
    DefaultBinding { id: "layer.live", keys: &["ctrl-l"] },
    DefaultBinding { id: "history.clear", keys: &["ctrl-k"] },
];
