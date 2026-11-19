//! `nu`'s actions and default keys (see `docs/usage-design.md`): both panes share the pane
//! switching and quit (movement, palette and help come from the shell); each pane adds its own.

use ui::keys::{Action, action, basic};

/// Switching panes and quitting. The shell adds movement, the palette and the help.
const COMMON: &[Action] = &[
    basic("focus.next", "next pane", "focus the next pane", &["tab"]),
    basic("focus.prev", "previous pane", "focus the previous pane", &["backtab"]),
    basic("focus.wifi", "Wi-Fi pane", "focus the Wi-Fi pane", &["1"]),
    basic("focus.bluetooth", "Bluetooth pane", "focus the Bluetooth pane", &["2"]),
    basic("quit", "quit", "close nu", &["q", "esc", "ctrl-c", "leader q"]),
];

/// The actions only the Wi-Fi pane has.
const WIFI: &[Action] = &[
    action(
        "wifi.connect",
        "connect",
        "connect to the network (asks for the password of a new protected one); on the connected network, disconnect",
        &["enter", "leader c"],
    ),
    action(
        "wifi.disconnect",
        "disconnect",
        "disconnect from the current network",
        &["x", "leader x"],
    ),
    action(
        "wifi.forget",
        "forget",
        "delete the saved connection of the network (asks first)",
        &["d", "leader d"],
    ),
    action("wifi.scan", "scan", "ask the radio for a fresh scan", &["s", "leader s"]),
    action("wifi.power", "toggle Wi-Fi", "turn the Wi-Fi radio on or off", &["leader p"]),
];

/// The actions only the Bluetooth pane has.
const BLUETOOTH: &[Action] = &[
    action(
        "bt.connect",
        "connect",
        "connect to the device (pairs and trusts it first if it is new); on a connected device, disconnect",
        &["enter", "leader c"],
    ),
    action("bt.disconnect", "disconnect", "disconnect the device", &["x", "leader x"]),
    action(
        "bt.remove",
        "forget",
        "unpair the device and delete what is known about it (asks first)",
        &["d", "leader d"],
    ),
    action(
        "bt.trust",
        "toggle trust",
        "trust the device (it may connect by itself) or stop trusting it",
        &["t", "leader t"],
    ),
    action("bt.scan", "scan", "start or stop looking for devices", &["s", "leader s"]),
    action("bt.power", "toggle Bluetooth", "turn the Bluetooth adapter on or off", &["leader p"]),
];

/// The actions of both panes.
pub fn common() -> Vec<Action> {
    COMMON.to_vec()
}

/// The actions of the Wi-Fi (0) and Bluetooth (1) panes, one set each.
pub fn panes() -> Vec<Vec<Action>> {
    vec![WIFI.to_vec(), BLUETOOTH.to_vec()]
}

#[cfg(test)]
mod tests;
