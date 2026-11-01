//! `nu`'s actions and default keys (see `docs/usage-design.md`).

use ui::keys::{Action, action, basic};

pub const ACTIONS: &[Action] = &[
    basic("nav.up", "up", "move the cursor up", &["k", "up", "ctrl-p"]),
    basic("nav.down", "down", "move the cursor down", &["j", "down", "ctrl-n"]),
    basic("nav.page_up", "page up", "move a page up", &["pageup"]),
    basic("nav.page_down", "page down", "move a page down", &["pagedown"]),
    basic("nav.top", "top", "go to the first network", &["g", "home"]),
    basic("nav.bottom", "bottom", "go to the last network", &["G", "end"]),
    action("wifi.connect", "connect", "connect to the network (asks for the password of a new protected one); on the connected network, disconnect", &["enter", "leader c"]),
    action("wifi.disconnect", "disconnect", "disconnect from the current network", &["x", "leader x"]),
    action("wifi.forget", "forget", "delete the saved connection of the network (asks first)", &["d", "leader d"]),
    action("wifi.scan", "scan", "ask the radio for a fresh scan", &["s", "leader s"]),
    action("wifi.power", "toggle Wi-Fi", "turn the Wi-Fi radio on or off", &["leader p"]),
    basic("palette", "commands", "search every action by name and run it", &["leader space"]),
    basic("help", "keys", "list every action with its keys", &["?"]),
    basic("quit", "quit", "close nu", &["q", "esc", "ctrl-c", "leader q"]),
];

#[cfg(test)]
mod tests {
    use super::*;
    use ui::keys::Keymap;

    #[test]
    fn every_action_is_described_and_no_two_share_a_key() {
        let mut ids = std::collections::HashSet::new();
        let mut keys = std::collections::HashSet::new();
        for a in ACTIONS {
            assert!(!a.name.is_empty() && !a.desc.is_empty() && !a.keys.is_empty(), "{}", a.id);
            assert!(ids.insert(a.id), "duplicate id {}", a.id);
            for k in a.keys {
                assert!(keys.insert(*k), "key {k} bound twice");
            }
        }
        let map = Keymap::new("space", ACTIONS, &[]);
        for a in ACTIONS {
            assert!(!map.keys_of(a.id).is_empty(), "{} has no parsable key", a.id);
        }
    }
}
