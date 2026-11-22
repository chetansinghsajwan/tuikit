//! The default keys of `gu`'s actions.

use ui::keys::DefaultBinding;

/// The default keys of [`GLOBAL`].
pub(super) const GLOBAL: &[DefaultBinding] = &[
    DefaultBinding { id: "focus.next", keys: &["tab"] },
    DefaultBinding { id: "focus.prev", keys: &["backtab"] },
    DefaultBinding { id: "focus.files", keys: &["1"] },
    DefaultBinding { id: "focus.branches", keys: &["3"] },
    DefaultBinding { id: "focus.commits", keys: &["2"] },
    DefaultBinding { id: "focus.stashes", keys: &["4"] },
    DefaultBinding { id: "focus.submodules", keys: &["5"] },
    DefaultBinding { id: "repo.up", keys: &["leader U"] },
    DefaultBinding { id: "remote.fetch", keys: &["leader r f"] },
    DefaultBinding { id: "remote.pull", keys: &["leader r p"] },
    DefaultBinding { id: "remote.push", keys: &["leader r P"] },
    DefaultBinding { id: "remote.force_push", keys: &["leader r F"] },
    DefaultBinding { id: "switch.to", keys: &["leader S"] },
    DefaultBinding { id: "op.continue", keys: &["leader o c"] },
    DefaultBinding { id: "op.abort", keys: &["leader o a"] },
];

/// The default keys of [`COMMON`].
pub(super) const COMMON: &[DefaultBinding] = &[
    DefaultBinding { id: "preview.down", keys: &["ctrl-d"] },
    DefaultBinding { id: "preview.up", keys: &["ctrl-u"] },
    DefaultBinding { id: "preview.next_hunk", keys: &["}"] },
    DefaultBinding { id: "preview.prev_hunk", keys: &["{"] },
    DefaultBinding { id: "preview.next_file", keys: &[")"] },
    DefaultBinding { id: "preview.prev_file", keys: &["("] },
    DefaultBinding { id: "preview.fold", keys: &["z"] },
    DefaultBinding { id: "preview.whitespace", keys: &["w"] },
    DefaultBinding { id: "back", keys: &["esc"] },
    DefaultBinding { id: "quit", keys: &["q", "ctrl-c", "leader q"] },
];

/// The default keys of [`BRANCHES`].
pub(super) const BRANCHES: &[DefaultBinding] = &[
    DefaultBinding { id: "branch.checkout", keys: &["enter", "leader k"] },
    DefaultBinding { id: "branch.new", keys: &["n", "leader n"] },
    DefaultBinding { id: "branch.delete", keys: &["d", "leader d"] },
    DefaultBinding { id: "branch.force_delete", keys: &["D"] },
    DefaultBinding { id: "branch.merge", keys: &["M", "leader m"] },
    DefaultBinding { id: "branch.rebase", keys: &["B", "leader b"] },
    DefaultBinding { id: "branch.rename", keys: &["r", "leader e"] },
];

/// The default keys of [`COMMITS`].
pub(super) const COMMITS: &[DefaultBinding] = &[
    DefaultBinding { id: "commit.files", keys: &["l", "right", "leader l"] },
    DefaultBinding { id: "commit.copy_hash", keys: &["y", "leader y"] },
    DefaultBinding { id: "commit.checkout", keys: &["enter", "leader k"] },
    DefaultBinding { id: "search.start", keys: &["/", "leader s"] },
    DefaultBinding { id: "search.next", keys: &["n"] },
    DefaultBinding { id: "search.prev", keys: &["N"] },
    DefaultBinding { id: "filter.open", keys: &["f", "leader f"] },
    DefaultBinding { id: "filter.clear", keys: &["F", "leader F"] },
    DefaultBinding { id: "view.graph", keys: &["v g"] },
    DefaultBinding { id: "view.hash", keys: &["v h"] },
    DefaultBinding { id: "view.author", keys: &["v a"] },
    DefaultBinding { id: "view.author_name", keys: &["v n"] },
    DefaultBinding { id: "view.refs", keys: &["v r"] },
    DefaultBinding { id: "view.subject", keys: &["v m"] },
    DefaultBinding { id: "view.rel_time", keys: &["v t"] },
    DefaultBinding { id: "view.abs_time", keys: &["v d"] },
    DefaultBinding { id: "view.all", keys: &["v b"] },
    DefaultBinding { id: "view.detail", keys: &["v p", "p"] },
    DefaultBinding { id: "commit.compare_work", keys: &["M"] },
    DefaultBinding { id: "commit.compare", keys: &["m"] },
    DefaultBinding { id: "commit.cherry_pick", keys: &["C", "leader c"] },
    DefaultBinding { id: "commit.revert", keys: &["R", "leader v"] },
    DefaultBinding { id: "commit.amend", keys: &["A", "leader A"] },
    DefaultBinding { id: "commit.reword", keys: &["r", "leader w"] },
    DefaultBinding { id: "commit.branch", keys: &["b", "leader n"] },
    DefaultBinding { id: "commit.tag", keys: &["t", "leader t"] },
    DefaultBinding { id: "commit.reset_soft", keys: &["leader z s"] },
    DefaultBinding { id: "commit.reset_mixed", keys: &["leader z m"] },
    DefaultBinding { id: "commit.reset_hard", keys: &["leader z h"] },
];

/// The default keys of [`STASHES`].
pub(super) const STASHES: &[DefaultBinding] = &[
    DefaultBinding { id: "stash.new", keys: &["n", "leader n"] },
    DefaultBinding { id: "stash.apply", keys: &["a", "enter", "leader a"] },
    DefaultBinding { id: "stash.pop", keys: &["p", "leader p"] },
    DefaultBinding { id: "stash.drop", keys: &["d", "leader d"] },
];

/// The default keys of [`SUBMODULES`].
pub(super) const SUBMODULES: &[DefaultBinding] = &[
    DefaultBinding { id: "submodule.open", keys: &["enter", "l", "right", "leader k"] },
    DefaultBinding { id: "submodule.update", keys: &["u", "leader a"] },
    DefaultBinding { id: "submodule.update_all", keys: &["U"] },
    DefaultBinding { id: "submodule.update_remote", keys: &["r", "leader p"] },
    DefaultBinding { id: "submodule.sync", keys: &["s"] },
    DefaultBinding { id: "submodule.add", keys: &["n", "a", "leader n"] },
    DefaultBinding { id: "submodule.remove", keys: &["d", "leader d"] },
];
