//! Checks that the action tables are complete and conflict-free.

use super::*;

#[test]
fn every_pane_has_described_actions_and_no_shared_keys() {
    for (pane, own) in panes().into_iter().enumerate() {
        let mut all = common();
        all.extend(&own);
        assert_eq!(all.problems(), Vec::<String>::new(), "pane {pane}");
    }
}
