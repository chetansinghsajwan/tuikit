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
    DefaultBinding { id: "panel.cpu", keys: &["1", "leader w c"] },
    DefaultBinding { id: "panel.memory", keys: &["2", "leader w m"] },
    DefaultBinding { id: "panel.network", keys: &["3", "leader w n"] },
    DefaultBinding { id: "panel.processes", keys: &["4", "leader w p"] },
    DefaultBinding { id: "panel.sensors", keys: &["5", "leader w s"] },
    DefaultBinding { id: "quit", keys: &["q", "ctrl-c"] },
];
