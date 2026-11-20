//! Tests for the action table: every action is described and no key is bound twice.

use super::*;
use ui::keys::Keymap;

#[test]
fn every_action_is_described_and_no_two_share_a_key() {
    let mut ids = std::collections::HashSet::new();
    for a in ACTIONS.iter() {
        assert!(!a.name.is_empty() && !a.desc.is_empty() && !a.keys.is_empty(), "{}", a.id);
        assert!(ids.insert(a.id), "duplicate id {}", a.id);
    }
    // Building the keymap parses every key; none may be dropped or shared.
    let map = Keymap::new("space", &ACTIONS, GROUPS);
    for a in ACTIONS.iter() {
        assert!(!map.keys_of(a.id).is_empty(), "{} has no parsable key", a.id);
    }
    let mut shown = std::collections::HashSet::new();
    for a in ACTIONS.iter() {
        for k in a.keys {
            assert!(shown.insert(*k), "key {k} bound twice");
        }
    }
}

#[test]
fn a_host_owning_the_keymap_gets_everything_but_what_the_shell_supplies() {
    let hosted = hosted();
    for a in &hosted {
        assert!(!a.id.starts_with("nav.") && !matches!(a.id, "palette" | "help"), "{}", a.id);
        assert!(owns(a.id), "{} is not the panel's", a.id);
    }
    for id in ["item.rename", "tree.out", "back", "quit", "search.files"] {
        assert!(hosted.iter().any(|a| a.id == id), "{id} is missing");
    }
}
