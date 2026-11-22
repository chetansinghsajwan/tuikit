//! The default keys of `nu`'s actions.

use ui::keys::DefaultBinding;

/// The default keys of [`COMMON`].
pub(super) const COMMON: &[DefaultBinding] = &[
    DefaultBinding { id: "focus.next", keys: &["tab"] },
    DefaultBinding { id: "focus.prev", keys: &["backtab"] },
    DefaultBinding { id: "focus.wifi", keys: &["1"] },
    DefaultBinding { id: "focus.bluetooth", keys: &["2"] },
    DefaultBinding { id: "quit", keys: &["q", "esc", "ctrl-c", "leader q"] },
];

/// The default keys of [`WIFI`].
pub(super) const WIFI: &[DefaultBinding] = &[
    DefaultBinding { id: "wifi.connect", keys: &["enter", "leader c"] },
    DefaultBinding { id: "wifi.disconnect", keys: &["x", "leader x"] },
    DefaultBinding { id: "wifi.forget", keys: &["d", "leader d"] },
    DefaultBinding { id: "wifi.scan", keys: &["s", "leader s"] },
    DefaultBinding { id: "wifi.power", keys: &["leader p"] },
];

/// The default keys of [`BLUETOOTH`].
pub(super) const BLUETOOTH: &[DefaultBinding] = &[
    DefaultBinding { id: "bt.connect", keys: &["enter", "leader c"] },
    DefaultBinding { id: "bt.disconnect", keys: &["x", "leader x"] },
    DefaultBinding { id: "bt.remove", keys: &["d", "leader d"] },
    DefaultBinding { id: "bt.trust", keys: &["t", "leader t"] },
    DefaultBinding { id: "bt.scan", keys: &["s", "leader s"] },
    DefaultBinding { id: "bt.power", keys: &["leader p"] },
];
