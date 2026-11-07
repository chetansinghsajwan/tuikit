//! The kinds of things a picker holds: modes, options, actions, the preview.

use super::*;

/// How the query is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Fuzzy-filter the candidates pushed through `injector()`.
    Fuzzy,
    /// The query is a regex searched for in the files under `root`; matches are
    /// the candidates.
    Grep,
}

/// The line that takes the typing in grep mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Field {
    Query,
    Include,
    Exclude,
}

#[derive(Debug, Clone)]
pub struct PickerOptions {
    pub mode: Mode,
    /// Candidates are paths relative to `root`: path-aware scoring, preview and git state.
    /// Always true in `Mode::Grep`.
    pub paths: bool,
    pub root: PathBuf,
    pub preview: bool,
    pub git: bool,
    pub query: String,
}

/// What the host should do after an event.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    /// The user accepted: the marked candidates, or the highlighted one
    /// (`Candidate::output` form).
    Accept(Vec<String>),
    /// One of the `--expect` keys was pressed: its name and the selection
    /// (like `Accept`). The host runs the action tied to the key.
    AcceptWith(String, Vec<String>),
    Cancel,
}

pub(super) struct Preview {
    pub(super) key: (String, u32, u16, u16),
    pub(super) lines: Vec<Line<'static>>,
}

/// A candidate source that runs again for each query (a language server answering as you type).
/// It gets the query, where to push candidates, and a flag that is set when the query changes again.
pub type LiveSource = Box<dyn Fn(&str, Injector<Candidate>, Arc<AtomicBool>) + Send + Sync>;

/// How the rows of a text list look: highlighted as code, numbered, with the age of each.
#[derive(Debug, Clone, Default)]
pub struct RowStyle {
    pub syntax: Option<String>,
    pub numbers: bool,
}
