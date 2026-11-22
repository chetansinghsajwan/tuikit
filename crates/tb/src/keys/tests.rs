//! Tests for the key table.

use super::*;

#[test]
fn every_action_is_described_and_every_key_parses_and_is_used_once() {
    assert_eq!(commands().problems(), Vec::<String>::new());
}
