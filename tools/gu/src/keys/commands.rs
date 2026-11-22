//! `gu`'s actions, per pane, and the names of its leader menus.

use ui::keys::{Action, Group};

/// Switching panes and the like: given to the Files panel too.
pub const GLOBAL: &[Action] = &[
    Action {
        id: "focus.next",
        name: "next pane",
        desc: "focus the next pane of the tab (Refs: branches and stashes)",
        basic: true,
    },
    Action {
        id: "focus.prev",
        name: "previous pane",
        desc: "focus the previous pane",
        basic: true,
    },
    Action {
        id: "focus.files",
        name: "status tab",
        desc: "show the Status tab (changed files)",
        basic: true,
    },
    Action {
        id: "focus.branches",
        name: "refs: branches",
        desc: "show the Refs tab on the branches, tags and remotes",
        basic: true,
    },
    Action {
        id: "focus.commits",
        name: "commits tab",
        desc: "show the Commits tab (the full graph)",
        basic: true,
    },
    Action {
        id: "focus.stashes",
        name: "refs: stashes",
        desc: "show the Refs tab on the stashes",
        basic: true,
    },
    Action {
        id: "focus.submodules",
        name: "refs: submodules",
        desc: "show the Refs tab on the submodules",
        basic: true,
    },
    Action {
        id: "repo.up",
        name: "up to parent repo",
        desc: "go back to the repository this submodule was opened from",
        basic: false,
    },
    Action {
        id: "remote.fetch",
        name: "fetch",
        desc: "fetch every remote and prune what is gone there",
        basic: false,
    },
    Action {
        id: "remote.pull",
        name: "pull",
        desc: "pull the upstream into the checked-out branch",
        basic: false,
    },
    Action {
        id: "remote.push",
        name: "push",
        desc: "push the checked-out branch (the first push sets its upstream on origin)",
        basic: false,
    },
    Action {
        id: "remote.force_push",
        name: "force push",
        desc: "push with --force-with-lease (asks first)",
        basic: false,
    },
    Action {
        id: "switch.to",
        name: "switch to",
        desc: "check out a branch, tag, remote branch or any commit by hash (type, up and down choose, tab fills)",
        basic: false,
    },
    Action {
        id: "op.continue",
        name: "continue",
        desc: "continue the stopped merge, rebase, cherry-pick or revert once conflicts are resolved and staged",
        basic: false,
    },
    Action {
        id: "op.abort",
        name: "abort",
        desc: "give up the stopped merge, rebase, cherry-pick or revert",
        basic: false,
    },
];

/// Movement, the diff scroll and quitting, for the list panes.
pub(super) const COMMON: &[Action] = &[
    Action {
        id: "preview.down",
        name: "scroll diff down",
        desc: "scroll the preview down",
        basic: true,
    },
    Action { id: "preview.up", name: "scroll diff up", desc: "scroll the preview up", basic: true },
    Action {
        id: "preview.next_hunk",
        name: "next hunk",
        desc: "scroll the diff to the next hunk",
        basic: true,
    },
    Action {
        id: "preview.prev_hunk",
        name: "previous hunk",
        desc: "scroll the diff to the previous hunk",
        basic: true,
    },
    Action {
        id: "preview.next_file",
        name: "next file",
        desc: "scroll the diff to the next file",
        basic: true,
    },
    Action {
        id: "preview.prev_file",
        name: "previous file",
        desc: "scroll the diff to the previous file",
        basic: true,
    },
    Action {
        id: "preview.fold",
        name: "fold file",
        desc: "fold or unfold the file at the top of the diff (lockfiles and huge files start folded)",
        basic: false,
    },
    Action {
        id: "preview.whitespace",
        name: "ignore whitespace",
        desc: "show changes in whitespace only as unchanged lines",
        basic: false,
    },
    Action { id: "back", name: "back to files", desc: "focus the files pane", basic: true },
    Action { id: "quit", name: "quit", desc: "close gu", basic: true },
];

/// The actions of the Refs pane (branches, remotes and tags).
pub(super) const BRANCHES: &[Action] = &[
    Action {
        id: "branch.checkout",
        name: "check out",
        desc: "switch to the branch (a remote branch becomes a tracking branch, a tag detaches HEAD)",
        basic: false,
    },
    Action {
        id: "branch.new",
        name: "new branch",
        desc: "create a branch at HEAD and switch to it",
        basic: false,
    },
    Action {
        id: "branch.delete",
        name: "delete",
        desc: "delete the local branch, tag or remote branch (asks first)",
        basic: false,
    },
    Action {
        id: "branch.force_delete",
        name: "force-delete branch",
        desc: "delete the local branch even if it is not merged (asks first)",
        basic: false,
    },
    Action {
        id: "branch.merge",
        name: "merge",
        desc: "merge the branch into the checked-out one (asks first)",
        basic: false,
    },
    Action {
        id: "branch.rebase",
        name: "rebase onto",
        desc: "replay the checked-out branch on top of this one (asks first)",
        basic: false,
    },
    Action {
        id: "branch.rename",
        name: "rename",
        desc: "rename the branch, tag, remote branch (pushes the new name, deletes the old), remote or folder under the cursor",
        basic: false,
    },
];

/// The actions of the Commits pane.
pub(super) const COMMITS: &[Action] = &[
    Action {
        id: "commit.files",
        name: "files",
        desc: "explore the files of the commit as a tree, with the diff of each (h or esc goes back)",
        basic: false,
    },
    Action {
        id: "commit.copy_hash",
        name: "copy hash",
        desc: "copy the full hash of the commit to the clipboard",
        basic: false,
    },
    Action {
        id: "commit.checkout",
        name: "check out",
        desc: "check out the commit (detached HEAD)",
        basic: false,
    },
    Action {
        id: "search.start",
        name: "search",
        desc: "search the commits by hash, subject, author or branch and tag names",
        basic: false,
    },
    Action {
        id: "search.next",
        name: "next match",
        desc: "go to the next commit that matches the search",
        basic: true,
    },
    Action {
        id: "search.prev",
        name: "previous match",
        desc: "go to the previous commit that matches the search",
        basic: true,
    },
    Action {
        id: "filter.open",
        name: "filter",
        desc: "choose what to show: message, authors, categories (fix, feat), branches, date (several of each)",
        basic: false,
    },
    Action {
        id: "filter.clear",
        name: "clear filters",
        desc: "show every commit again",
        basic: false,
    },
    Action {
        id: "view.graph",
        name: "graph",
        desc: "show or hide the commit graph (lanes, merges)",
        basic: false,
    },
    Action { id: "view.hash", name: "hash", desc: "show or hide the hash column", basic: false },
    Action {
        id: "view.author",
        name: "author",
        desc: "show or hide the author column",
        basic: false,
    },
    Action {
        id: "view.author_name",
        name: "author name",
        desc: "show the authors as initials or as full names",
        basic: false,
    },
    Action {
        id: "view.refs",
        name: "refs",
        desc: "show or hide the branch and tag pills",
        basic: false,
    },
    Action {
        id: "view.subject",
        name: "message",
        desc: "show or hide the first line of the message",
        basic: false,
    },
    Action {
        id: "view.rel_time",
        name: "relative time",
        desc: "show or hide how long ago the commit was made (3d)",
        basic: false,
    },
    Action {
        id: "view.abs_time",
        name: "date",
        desc: "show or hide the date and time of the commit",
        basic: false,
    },
    Action {
        id: "view.all",
        name: "all branches",
        desc: "show the commits of every branch, not only the checked-out one",
        basic: false,
    },
    Action {
        id: "view.detail",
        name: "diff",
        desc: "show or hide the diff beside the list",
        basic: false,
    },
    Action {
        id: "commit.compare_work",
        name: "compare with work tree",
        desc: "show the diff from this commit to the current state of the files (changes not yet committed included)",
        basic: false,
    },
    Action {
        id: "commit.compare",
        name: "compare",
        desc: "mark this commit, then move to another to see the diff between the two (again to clear)",
        basic: false,
    },
    Action {
        id: "commit.cherry_pick",
        name: "cherry-pick",
        desc: "apply the commit on top of HEAD",
        basic: false,
    },
    Action {
        id: "commit.revert",
        name: "revert",
        desc: "add a commit that undoes this one (asks first)",
        basic: false,
    },
    Action {
        id: "commit.amend",
        name: "amend",
        desc: "add what is staged to the last commit and edit its message (enter keeps it)",
        basic: false,
    },
    Action {
        id: "commit.reword",
        name: "reword",
        desc: "change only the message of a commit (later commits are rebuilt; files untouched)",
        basic: false,
    },
    Action {
        id: "commit.branch",
        name: "new branch here",
        desc: "create a branch at the commit and switch to it",
        basic: false,
    },
    Action { id: "commit.tag", name: "tag", desc: "create a tag at the commit", basic: false },
    Action {
        id: "commit.reset_soft",
        name: "reset soft",
        desc: "move the branch here, keep the index and the work tree",
        basic: false,
    },
    Action {
        id: "commit.reset_mixed",
        name: "reset mixed",
        desc: "move the branch here, keep the work tree",
        basic: false,
    },
    Action {
        id: "commit.reset_hard",
        name: "reset hard",
        desc: "move the branch here and throw changes away (asks first)",
        basic: false,
    },
];

/// The actions of the Stashes pane.
pub(super) const STASHES: &[Action] = &[
    Action {
        id: "stash.new",
        name: "new stash",
        desc: "stash the changes with a message",
        basic: false,
    },
    Action {
        id: "stash.apply",
        name: "apply stash",
        desc: "apply the stash and keep it",
        basic: false,
    },
    Action {
        id: "stash.pop",
        name: "pop stash",
        desc: "apply the stash and drop it",
        basic: false,
    },
    Action {
        id: "stash.drop",
        name: "drop stash",
        desc: "drop the stash (asks first)",
        basic: false,
    },
];

/// The actions of the Submodules pane.
pub(super) const SUBMODULES: &[Action] = &[
    Action {
        id: "submodule.open",
        name: "open",
        desc: "open the submodule as the repository on screen (leader U goes back up); a submodule not cloned yet is cloned first",
        basic: false,
    },
    Action {
        id: "submodule.update",
        name: "update",
        desc: "clone if needed and check out the commit the super-project records",
        basic: false,
    },
    Action {
        id: "submodule.update_all",
        name: "update all",
        desc: "update every submodule",
        basic: false,
    },
    Action {
        id: "submodule.update_remote",
        name: "update to remote tip",
        desc: "check out the latest commit of the submodule's remote branch (shows as a change to stage)",
        basic: false,
    },
    Action {
        id: "submodule.sync",
        name: "sync urls",
        desc: "copy the URLs of .gitmodules to the config of the repo and the submodules",
        basic: false,
    },
    Action {
        id: "submodule.add",
        name: "add",
        desc: "add a submodule: the URL, then optionally a space and the path",
        basic: false,
    },
    Action {
        id: "submodule.remove",
        name: "remove",
        desc: "remove the submodule and delete its work tree (asks first)",
        basic: false,
    },
];

/// Names for the menus behind `leader r` and `leader o` (and `leader z` in Commits).
pub const GROUPS: &[Group] = &[
    Group { prefix: "leader r", name: "remote" },
    Group { prefix: "leader o", name: "operation" },
    Group { prefix: "leader z", name: "reset" },
    Group { prefix: "v", name: "view" },
];
