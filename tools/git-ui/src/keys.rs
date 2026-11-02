//! `git-ui`'s actions and default keys (see `docs/usage-design.md`). The Files pane is the
//! `git-panel` panel with its own table; the others share movement, the global actions and
//! the common ones, plus what is specific to the pane.

use ui::keys::{Action, Group, action, basic};

/// Switching panes and the like: given to the Files panel too.
pub const GLOBAL: &[Action] = &[
    basic("tab.next", "next tab", "go to the next tab (Status, Commits, Refs)", &["]"]),
    basic("tab.prev", "previous tab", "go to the previous tab", &["["]),
    basic("focus.next", "next pane", "focus the next pane of the tab (Refs: branches and stashes)", &["tab"]),
    basic("focus.prev", "previous pane", "focus the previous pane", &["backtab"]),
    basic("focus.files", "status tab", "show the Status tab (changed files)", &["1"]),
    basic("focus.branches", "refs: branches", "show the Refs tab on the branches, tags and remotes", &["2"]),
    basic("focus.commits", "commits tab", "show the Commits tab (the full graph)", &["3"]),
    basic("focus.stashes", "refs: stashes", "show the Refs tab on the stashes", &["4"]),
    action("remote.fetch", "fetch", "fetch every remote and prune what is gone there", &["leader r f"]),
    action("remote.pull", "pull", "pull the upstream into the checked-out branch", &["leader r p"]),
    action("remote.push", "push", "push the checked-out branch (the first push sets its upstream on origin)", &["leader r P"]),
    action("remote.force_push", "force push", "push with --force-with-lease (asks first)", &["leader r F"]),
    action("op.continue", "continue", "continue the stopped merge, rebase, cherry-pick or revert once conflicts are resolved and staged", &["leader o c"]),
    action("op.abort", "abort", "give up the stopped merge, rebase, cherry-pick or revert", &["leader o a"]),
];

/// Names for the menus behind `leader r` and `leader o` (and `leader z` in Commits).
pub const GROUPS: &[Group] = &[
    Group { prefix: "leader r", name: "remote" },
    Group { prefix: "leader o", name: "operation" },
    Group { prefix: "leader z", name: "reset" },
    Group { prefix: "leader f", name: "filter" },
];

/// Movement, the diff scroll and quitting, for the list panes.
const COMMON: &[Action] = &[
    basic("nav.up", "up", "move the cursor up", &["k", "up", "ctrl-p"]),
    basic("nav.down", "down", "move the cursor down", &["j", "down", "ctrl-n"]),
    basic("nav.up5", "up 5", "move the cursor up five rows", &["ctrl-k"]),
    basic("nav.down5", "down 5", "move the cursor down five rows", &["ctrl-j"]),
    basic("nav.page_up", "page up", "move a page up", &["pageup"]),
    basic("nav.page_down", "page down", "move a page down", &["pagedown"]),
    basic("nav.top", "top", "go to the first row", &["g", "home"]),
    basic("nav.bottom", "bottom", "go to the last row", &["G", "end"]),
    basic("preview.down", "scroll diff down", "scroll the preview down", &["ctrl-d"]),
    basic("preview.up", "scroll diff up", "scroll the preview up", &["ctrl-u"]),
    basic("back", "back to files", "focus the files pane", &["esc"]),
    basic("palette", "commands", "search every action by name and run it", &["leader space"]),
    basic("help", "keys", "list every action with its keys", &["?"]),
    basic("quit", "quit", "close git-ui", &["q", "ctrl-c", "leader q"]),
];

const BRANCHES: &[Action] = &[
    action("branch.checkout", "check out", "switch to the branch (a remote branch becomes a tracking branch, a tag detaches HEAD)", &["enter", "leader k"]),
    action("branch.new", "new branch", "create a branch at HEAD and switch to it", &["n", "leader n"]),
    action("branch.delete", "delete", "delete the local branch, tag or remote branch (asks first)", &["d", "leader d"]),
    action("branch.force_delete", "force-delete branch", "delete the local branch even if it is not merged (asks first)", &["D"]),
    action("branch.merge", "merge", "merge the branch into the checked-out one (asks first)", &["M", "leader m"]),
    action("branch.rebase", "rebase onto", "replay the checked-out branch on top of this one (asks first)", &["B", "leader b"]),
    action("branch.rename", "rename", "rename the local branch", &["R", "leader e"]),
];

const COMMITS: &[Action] = &[
    action("commit.checkout", "check out", "check out the commit (detached HEAD)", &["enter", "leader k"]),
    action("search.start", "search", "search the commits by hash, subject, author or branch and tag names", &["/", "leader s"]),
    basic("search.next", "next match", "go to the next commit that matches the search", &["n"]),
    basic("search.prev", "previous match", "go to the previous commit that matches the search", &["N"]),
    action("view.detail", "diff", "show or hide the diff beside the list (the Commits tab)", &["p"]),
    action("filter.text", "filter by message", "show only commits whose message contains the text (empty clears)", &["leader f m"]),
    action("filter.author", "filter by author", "show only commits by authors whose name contains the text (empty clears)", &["leader f a"]),
    action("filter.branch", "filter by branch", "show only the history of a branch or tag (empty clears)", &["leader f b"]),
    action("filter.since", "filter by date", "show only commits since 7d, 2w, 3m, 1y or a date like 2026-10-01 (empty clears)", &["leader f d"]),
    action("filter.clear", "clear filters", "show every commit again", &["leader f x"]),
    action("view.graph", "graph", "show or hide the commit graph (lanes, merges, branch labels)", &["leader g"]),
    action("view.all", "all branches", "show the commits of every branch, not only the checked-out one", &["leader a"]),
    action("commit.cherry_pick", "cherry-pick", "apply the commit on top of HEAD", &["C", "leader c"]),
    action("commit.revert", "revert", "add a commit that undoes this one (asks first)", &["R", "leader v"]),
    action("commit.reword", "reword", "change the message of the last commit", &["w", "leader w"]),
    action("commit.branch", "new branch here", "create a branch at the commit and switch to it", &["b", "leader n"]),
    action("commit.tag", "tag", "create a tag at the commit", &["t", "leader t"]),
    action("commit.reset_soft", "reset soft", "move the branch here, keep the index and the work tree", &["leader z s"]),
    action("commit.reset_mixed", "reset mixed", "move the branch here, keep the work tree", &["leader z m"]),
    action("commit.reset_hard", "reset hard", "move the branch here and throw changes away (asks first)", &["leader z h"]),
];

const STASHES: &[Action] = &[
    action("stash.new", "new stash", "stash the changes with a message", &["n", "leader n"]),
    action("stash.apply", "apply stash", "apply the stash and keep it", &["a", "enter", "leader a"]),
    action("stash.pop", "pop stash", "apply the stash and drop it", &["p", "leader p"]),
    action("stash.drop", "drop stash", "drop the stash (asks first)", &["d", "leader d"]),
];

/// The actions of the Branches, Commits and Stashes panes (0, 1, 2).
pub fn pane_actions(pane: usize) -> Vec<Action> {
    let own: &[Action] = match pane {
        0 => BRANCHES,
        1 => COMMITS,
        2 => STASHES,
        _ => &[],
    };
    own.iter().chain(COMMON).chain(GLOBAL).cloned().collect()
}

#[cfg(test)]
mod tests {
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
}
