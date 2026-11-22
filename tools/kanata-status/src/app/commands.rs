//! The few actions the screen offers.

use ui::keys::Action;

/// The few keys the screen binds. All are Ctrl chords, and the shell's leader is one too, because
/// kanata passes every key typed here on to the system as well, so a plain key must not do
/// anything by accident.
pub const ACTIONS: &[Action] = &[
    Action { id: "quit", name: "quit", desc: "close kanata-status", basic: true },
    Action {
        id: "layer.next",
        name: "next layer",
        desc: "look at the next layer of the config",
        basic: true,
    },
    Action {
        id: "layer.prev",
        name: "previous layer",
        desc: "look at the previous layer of the config",
        basic: true,
    },
    Action {
        id: "layer.live",
        name: "live layer",
        desc: "follow the layer kanata is on again",
        basic: true,
    },
    Action {
        id: "history.clear",
        name: "clear history",
        desc: "empty the history log",
        basic: true,
    },
];
