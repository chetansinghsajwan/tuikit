use super::*;
use ui::keys::Keymap;

#[test]
fn every_action_is_described_and_no_two_share_a_key() {
    let mut ids = std::collections::HashSet::new();
    for a in ACTIONS {
        assert!(!a.name.is_empty() && !a.desc.is_empty() && !a.keys.is_empty(), "{}", a.id);
        assert!(ids.insert(a.id), "duplicate id {}", a.id);
    }
    // Building the keymap parses every key; none may be dropped or shared.
    let map = Keymap::new("space", ACTIONS, GROUPS);
    for a in ACTIONS {
        assert!(!map.keys_of(a.id).is_empty(), "{} has no parsable key", a.id);
    }
    let mut shown = std::collections::HashSet::new();
    for a in ACTIONS {
        for k in a.keys {
            assert!(shown.insert(*k), "key {k} bound twice");
        }
    }
}
