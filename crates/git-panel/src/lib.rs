//! `git-panel`: the shared git status panel (lazygit's main screen): the
//! changed files in sections, the diff of the highlighted one, and stage,
//! unstage, discard and commit. A library that editors embed as a popup, plus a
//! standalone binary. Reads use `git-core` (gix); changes run the `git` command
//! line through `git-core`.

/// The panel's actions and default keys.
pub mod keys;
/// The panel itself: state, key handling and drawing.
mod panel;

pub use panel::{Outcome, Panel};

/// The old name of `Outcome`, kept so hosts that still match on it keep building.
#[deprecated(note = "renamed to `Outcome`")]
pub type Action = Outcome;
