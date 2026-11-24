//! The default keys of the monitor's actions.

use ui::keys::DefaultBinding;

/// The default keys of [`ACTIONS`](super::ACTIONS).
pub const DEFAULT_BINDINGS: &[DefaultBinding] = &[
    DefaultBinding { id: "sort.next", keys: &["s", "leader s n"] },
    DefaultBinding { id: "sort.reverse", keys: &["r", "leader s r"] },
    DefaultBinding { id: "view.tree", keys: &["t", "leader v t"] },
    DefaultBinding { id: "filter.start", keys: &["/", "f"] },
    DefaultBinding { id: "filter.clear", keys: &["esc"] },
    DefaultBinding { id: "proc.term", keys: &["x", "leader p t"] },
    DefaultBinding { id: "proc.kill", keys: &["X", "leader p k"] },
    DefaultBinding { id: "quit", keys: &["q", "ctrl-c"] },
];
