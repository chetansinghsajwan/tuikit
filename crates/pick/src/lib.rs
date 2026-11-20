//! `pick`: the shared picker. A library (state, key handling, rendering through
//! `ui`) so other tools can embed it in-process, plus a thin CLI in `main.rs`.

mod candidate;
pub mod grep;
mod keys;
mod picker;
mod preview_cmd;
mod screen;
pub mod source;
mod tree;

pub use candidate::{Ancestor, Buffer, Candidate, GitItem, GitKind, Hier, Loc};
pub use picker::{Action, LiveSource, Mode, Picker, PickerOptions, PreviewFn, RowStyle};
pub use screen::PickScreen;
