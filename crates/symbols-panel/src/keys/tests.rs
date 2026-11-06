use super::*;
use ui::keys::Keymap;

#[test]
fn every_action_is_described_parsable_and_no_key_is_shared() {
    let map = Keymap::new("space", ACTIONS, GROUPS);
    let mut keys = std::collections::HashSet::new();
    for a in ACTIONS {
        assert!(!a.name.is_empty() && !a.desc.is_empty() && !a.keys.is_empty(), "{}", a.id);
        assert!(!map.keys_of(a.id).is_empty(), "{} has no parsable key", a.id);
        for k in a.keys {
            assert!(keys.insert(*k), "key {k} bound twice");
        }
    }
}
