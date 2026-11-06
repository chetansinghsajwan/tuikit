use super::*;
use ui::keys::Keymap;

#[test]
fn every_pane_has_described_actions_and_no_shared_keys() {
    for pane in 0..3 {
        let actions = pane_actions(pane);
        let mut ids = std::collections::HashSet::new();
        let mut keys = std::collections::HashSet::new();
        for a in &actions {
            assert!(!a.name.is_empty() && !a.desc.is_empty() && !a.keys.is_empty(), "{}", a.id);
            assert!(ids.insert(a.id), "duplicate id {} in pane {pane}", a.id);
            for k in a.keys {
                assert!(keys.insert(*k), "key {k} bound twice in pane {pane}");
            }
        }
        let map = Keymap::new("space", &actions, &[]);
        for a in &actions {
            assert!(!map.keys_of(a.id).is_empty(), "{} has no parsable key", a.id);
        }
    }
    // The global actions and the git panel's own table must not clash either.
    let panel: Vec<Action> = git_panel::keys::ACTIONS.iter().chain(GLOBAL).cloned().collect();
    let mut keys = std::collections::HashSet::new();
    for a in &panel {
        for k in a.keys {
            assert!(keys.insert(*k), "key {k} bound twice in the files pane");
        }
    }
}
