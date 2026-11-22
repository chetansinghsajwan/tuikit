//! Tests for the action table: every action is described and no key is bound twice.

use super::*;

#[test]
fn every_action_is_described_and_every_key_parses_and_is_used_once() {
    assert_eq!(COMMANDS.problems(), Vec::<String>::new());
}

#[test]
fn a_host_owning_the_keymap_gets_everything_but_what_the_shell_supplies() {
    let hosted = hosted_commands();
    for a in &hosted.actions {
        assert!(!a.id.starts_with("nav.") && !matches!(a.id, "palette" | "help"), "{}", a.id);
        assert!(is_own_action(a.id), "{} is not the panel's", a.id);
    }
    for id in ["item.rename", "tree.collapse", "back", "quit", "search.files"] {
        assert!(hosted.actions.iter().any(|a| a.id == id), "{id} is missing");
    }
}
