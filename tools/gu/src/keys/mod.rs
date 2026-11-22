//! `gu`'s actions and default keys (see `docs/usage-design.md`). The Files pane is the
//! `git-panel` panel with its own table; the others share movement, the global actions and
//! the common ones, plus what is specific to the pane.

use commands::{BRANCHES, COMMITS, COMMON, GLOBAL, STASHES, SUBMODULES};
use ui::keys::{Action, Commands, DefaultBinding};

/// The default keys of every pane's actions.
mod binds;
/// The actions of every pane and the names of the leader menus.
mod commands;

pub use commands::GROUPS;

/// The commands of the Branches, Commits, Stashes and Submodules panes (0, 1, 2, 3).
pub fn pane_commands(pane: usize) -> Commands {
    let (actions, keys): (&[Action], &[DefaultBinding]) = match pane {
        0 => (BRANCHES, binds::BRANCHES),
        1 => (COMMITS, binds::COMMITS),
        2 => (STASHES, binds::STASHES),
        3 => (SUBMODULES, binds::SUBMODULES),
        _ => (&[], &[]),
    };
    let mut set = Commands::of(actions, keys);
    set.add(COMMON, binds::COMMON);
    set.add(GLOBAL, binds::GLOBAL);
    set
}

/// The commands of the Files pane: the git panel's own and the global ones.
fn files_commands() -> Commands {
    let mut set = git_panel::keys::hosted_commands();
    set.add(GLOBAL, binds::GLOBAL);
    set
}

/// The commands of each context the shell switches between: Files (0), then Branches, Commits,
/// Stashes and Submodules (1 to 4).
pub fn contexts() -> Vec<Commands> {
    std::iter::once(files_commands()).chain((0..4).map(pane_commands)).collect()
}

/// Tests of the key tables.
#[cfg(test)]
mod tests;
