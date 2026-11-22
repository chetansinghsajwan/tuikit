//! `nu`'s actions, per pane.

use ui::keys::Action;

/// Switching panes and quitting. The shell adds movement, the palette and the help.
pub(super) const COMMON: &[Action] = &[
    Action { id: "focus.next", name: "next pane", desc: "focus the next pane", basic: true },
    Action {
        id: "focus.prev",
        name: "previous pane",
        desc: "focus the previous pane",
        basic: true,
    },
    Action { id: "focus.wifi", name: "Wi-Fi pane", desc: "focus the Wi-Fi pane", basic: true },
    Action {
        id: "focus.bluetooth",
        name: "Bluetooth pane",
        desc: "focus the Bluetooth pane",
        basic: true,
    },
    Action { id: "quit", name: "quit", desc: "close nu", basic: true },
];

/// The actions only the Wi-Fi pane has.
pub(super) const WIFI: &[Action] = &[
    Action {
        id: "wifi.connect",
        name: "connect",
        desc: "connect to the network (asks for the password of a new protected one); on the connected network, disconnect",
        basic: false,
    },
    Action {
        id: "wifi.disconnect",
        name: "disconnect",
        desc: "disconnect from the current network",
        basic: false,
    },
    Action {
        id: "wifi.forget",
        name: "forget",
        desc: "delete the saved connection of the network (asks first)",
        basic: false,
    },
    Action { id: "wifi.scan", name: "scan", desc: "ask the radio for a fresh scan", basic: false },
    Action {
        id: "wifi.power",
        name: "toggle Wi-Fi",
        desc: "turn the Wi-Fi radio on or off",
        basic: false,
    },
];

/// The actions only the Bluetooth pane has.
pub(super) const BLUETOOTH: &[Action] = &[
    Action {
        id: "bt.connect",
        name: "connect",
        desc: "connect to the device (pairs and trusts it first if it is new); on a connected device, disconnect",
        basic: false,
    },
    Action { id: "bt.disconnect", name: "disconnect", desc: "disconnect the device", basic: false },
    Action {
        id: "bt.remove",
        name: "forget",
        desc: "unpair the device and delete what is known about it (asks first)",
        basic: false,
    },
    Action {
        id: "bt.trust",
        name: "toggle trust",
        desc: "trust the device (it may connect by itself) or stop trusting it",
        basic: false,
    },
    Action { id: "bt.scan", name: "scan", desc: "start or stop looking for devices", basic: false },
    Action {
        id: "bt.power",
        name: "toggle Bluetooth",
        desc: "turn the Bluetooth adapter on or off",
        basic: false,
    },
];
