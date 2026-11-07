//! `nu`'s actions and default keys (see `docs/usage-design.md`): the keys of both panes share
//! movement and the global actions; each pane adds its own.

use ui::keys::{Action, action, basic};

/// Movement, switching panes, the palette and quitting.
const COMMON: &[Action] = &[
    basic("nav.up", "up", "move the cursor up", &["k", "up", "ctrl-p"]),
    basic("nav.down", "down", "move the cursor down", &["j", "down", "ctrl-n"]),
    basic("nav.up5", "up 5", "move the cursor up five rows", &["ctrl-k"]),
    basic("nav.down5", "down 5", "move the cursor down five rows", &["ctrl-j"]),
    basic("nav.page_up", "page up", "move a page up", &["pageup"]),
    basic("nav.page_down", "page down", "move a page down", &["pagedown"]),
    basic("nav.top", "top", "go to the first entry", &["g", "home"]),
    basic("nav.bottom", "bottom", "go to the last entry", &["G", "end"]),
    basic("focus.next", "next pane", "focus the next pane", &["tab"]),
    basic("focus.prev", "previous pane", "focus the previous pane", &["backtab"]),
    basic("focus.wifi", "Wi-Fi pane", "focus the Wi-Fi pane", &["1"]),
    basic("focus.bluetooth", "Bluetooth pane", "focus the Bluetooth pane", &["2"]),
    basic("palette", "commands", "search every action by name and run it", &["leader space"]),
    basic("help", "keys", "list every action with its keys", &["?"]),
    basic("quit", "quit", "close nu", &["q", "esc", "ctrl-c", "leader q"]),
];

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

/// The actions of the Wi-Fi (0) and Bluetooth (1) panes.
pub fn pane_actions(pane: usize) -> Vec<Action> {
    let own = if pane == 0 { WIFI } else { BLUETOOTH };
    own.iter().chain(COMMON).cloned().collect()
}

#[cfg(test)]
mod tests;
