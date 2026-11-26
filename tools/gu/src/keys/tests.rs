//! Checks that the pane key tables are complete and conflict-free.

use super::*;

#[test]
fn every_pane_has_described_actions_and_no_shared_keys() {
    for pane in 0..4 {
        assert_eq!(pane_commands(pane).problems(), Vec::<String>::new(), "pane {pane}");
    }
    // The global actions and the git panel's own table must not clash either.
    let mut files = git_panel::keys::commands();
    files.add(GLOBAL, binds::GLOBAL);
    assert_eq!(files.problems(), Vec::<String>::new(), "the files pane");
    assert_eq!(commit_files_commands().problems(), Vec::<String>::new(), "the files of a commit");
    assert_eq!(contexts().len(), COMMIT_FILES_CONTEXT + 1);
}
