//! The kinds of things a panel holds: options, modes, rows, prompts.

use super::*;

/// Settings a host chooses when it creates a panel.
pub struct PanelOptions {
    /// Show hidden files (dot files). Toggle with `.`.
    pub hidden: bool,
    /// Git status colors and signs.
    pub git: bool,
    /// What the columns beside each name show (cycle with `c`).
    pub columns: Columns,
    /// Where bookmarks are kept (`None`: in memory only).
    pub bookmarks: Option<PathBuf>,
    /// Pastes of at least this many bytes run in the background with a progress line
    /// (`None`: always at once).
    pub background_bytes: Option<u64>,
    /// The leader key (a key name, see `ui::keys`).
    pub leader: String,
    /// Actions of the host (tabs, a properties pane, ...) added to the keymap; the panel
    /// returns `Outcome::Command(id)` when one runs.
    pub commands: ui::keys::Commands,
}

impl Default for PanelOptions {
    fn default() -> Self {
        PanelOptions {
            hidden: true,
            git: true,
            columns: Columns::Off,
            bookmarks: bookmarks::default_path(),
            background_bytes: Some(8 << 20),
            leader: "space".into(),
            commands: ui::keys::Commands::default(),
        }
    }
}

/// What the columns beside each name show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Columns {
    Off,
    /// Permissions, size and modified time (the standalone app's default).
    Full,
    /// Size (folders are counted in the background) and modified time.
    SizeTime,
    /// Permissions and modified time.
    PermTime,
}

impl Columns {
    /// The next column layout in the cycle, so one key steps through all of them.
    pub fn cycle(self) -> Columns {
        match self {
            Columns::Off => Columns::Full,
            Columns::Full => Columns::SizeTime,
            Columns::SizeTime => Columns::PermTime,
            Columns::PermTime => Columns::Off,
        }
    }

    /// A short description of the layout, shown in the footer when it changes.
    pub(super) fn label(self) -> &'static str {
        match self {
            Columns::Off => "columns off",
            Columns::Full => "permissions, size and time",
            Columns::SizeTime => "size and time",
            Columns::PermTime => "permissions and time",
        }
    }
}

/// What the host should do after an event.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    /// The user chose a file to open.
    Open(PathBuf),
    /// The user asked to leave the panel (Esc, `q`).
    Close,
    /// A file or (`true`) folder was created.
    Created(PathBuf, bool),
    /// Files were renamed or moved: `(from, to)`. Open buffers should follow.
    Moved(Vec<(PathBuf, PathBuf)>),
    /// Files were trashed or deleted.
    Deleted(Vec<PathBuf>),
    /// The user asked to search: the host runs `pick` and calls `reveal` with the result.
    Search(Search),
    /// The user asked for the command palette: the host lists `actions()` with `ui::keys::choose`
    /// and runs the pick with `run`.
    Palette,
    /// The user asked for the keys help: the host shows `ui::keys::Help`.
    Help,
    /// An action the panel does not own (a host registered it in `PanelOptions::commands`): the host runs it.
    Command(&'static str),
    /// Rename these paths together by editing their names in a text editor.
    BulkRename(Vec<PathBuf>),
    /// Open this path with this command (typed in the `open with` prompt).
    OpenWith(PathBuf, String),
}

/// Whether a file has an execute bit (always false off Unix).
pub(super) fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

/// What to search for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Search {
    /// File names under the root.
    Files,
    /// Names in this folder only, not in its subfolders.
    Here(PathBuf),
    /// File contents under the root.
    Grep,
}

/// One visible line of the tree: a path with its depth and the facts the columns need.
pub(super) struct Row {
    pub(super) path: PathBuf,
    pub(super) name: String,
    pub(super) depth: u16,
    pub(super) is_dir: bool,
    pub(super) size: u64,
    pub(super) modified: Option<std::time::SystemTime>,
    pub(super) mode: u32,
    /// Git ignores it (only set while ignored files are shown).
    pub(super) ignored: bool,
}

/// Which text prompt is open and what it works on.
pub(super) enum PromptKind {
    /// Create inside this folder; a trailing `/` makes a folder.
    Create(PathBuf),
    Rename(PathBuf),
    OpenWith(PathBuf),
}

/// How a delete confirmation will remove things: to the trash or for good.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ConfirmKind {
    Trash,
    Delete,
}

impl ConfirmKind {
    /// Pressing the key that asked (`d`, `D`) again confirms.
    pub(super) fn confirm_key(self) -> char {
        match self {
            ConfirmKind::Trash => 'd',
            ConfirmKind::Delete => 'D',
        }
    }
}

/// Waiting for the key that names a bookmark.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BookmarkOp {
    Set,
    Jump,
    Delete,
}

/// What the panel is waiting for: a normal key, a bookmark key, typed text or a confirmation.
pub(super) enum Mode {
    Normal,
    Bookmark(BookmarkOp),
    Prompt(PromptKind, Input),
    Confirm(ConfirmKind, Vec<PathBuf>),
}
