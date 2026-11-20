//! The file browser panel: owns the tree, cursor, marks, prompts and background jobs, and turns
//! keys into `Outcome`s for the host. Drawing and paste live in the submodules.

use crate::bookmarks::{self, Bookmarks};
use crate::keys;
use crate::meta;
use crate::ops;
use git_core::{Kind, Repo, Status};
use ignore::WalkBuilder;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;
use ui::keys::{action, basic};
use ui::{Pane, Theme};

/// Bookmarks: folders saved under a key.
mod bookmark;
/// Actions on the row under the cursor or the marked rows.
mod item;
/// Movement: the cursor keys and folding.
mod nav;
/// Copy and move jobs that run in the background with progress.
mod paste;
/// Drawing of the tree, footer and prompts.
mod render;
/// Which folder is the root of the tree.
mod root;
/// Finding files and contents through the host's picker.
mod search;
/// The panel's small shared types (rows, modes, options, actions).
mod types;
/// What the list shows: hidden and ignored files, the columns, refresh.
mod view;
use paste::*;
use types::*;
pub use types::{Linemode, Outcome, PanelOptions, Search};

/// The actions that belong to no domain: the palette, the help, back and quit.
const GENERAL: &[ui::keys::Action] = &[
    basic("palette", "commands", "search every action by name and run it", &["leader space"]),
    action("help", "keys", "list every action with its keys", &["?"]),
    basic("back", "back", "clear the marks, else close", &["esc"]),
    basic("quit", "quit", "close the explorer", &["q", "ctrl-c", "leader q"]),
];

/// Every action of the panel in the order the help lists them: each domain's own table.
pub(crate) fn actions() -> Vec<ui::keys::Action> {
    [
        nav::ACTIONS,
        item::ACTIONS,
        search::ACTIONS,
        view::ACTIONS,
        root::ACTIONS,
        bookmark::ACTIONS,
        GENERAL,
    ]
    .concat()
}

#[cfg(test)]
mod tests;

/// The file browser state: one root folder shown as a tree, plus everything a key can change.
pub struct Panel {
    root: PathBuf,
    opts: PanelOptions,
    expanded: BTreeSet<PathBuf>,
    rows: Vec<Row>,
    cursor: usize,
    offset: usize,
    /// Rows visible at the last render, for paging.
    page: usize,
    git: Option<(Repo, Status)>,
    mode: Mode,
    marked: BTreeSet<PathBuf>,
    clip: Option<Clip>,
    /// Last result or error, shown in the footer until the next key.
    message: Option<(String, bool)>,
    bookmarks: Bookmarks,
    keymap: ui::keys::Keymap,
    /// Folder sizes counted in the background for the size column.
    dir_sizes: std::collections::HashMap<PathBuf, meta::DirSize>,
    /// What was last drawn for them, to know when a redraw is due.
    sizes_shown: Vec<(PathBuf, (u64, bool))>,
    /// A paste running in the background, and the action its end produced, waiting for the next event.
    job: Option<Job>,
    pending: Option<Outcome>,
    /// Something finished by itself since the last `tick`.
    redraw: bool,
    /// Show the entries that .gitignore and the other ignore files hide (toggle with `I`).
    show_ignored: bool,
}

impl Panel {
    /// Build a panel rooted at `root` and read the folders once, so it is ready to draw.
    pub fn new(root: impl Into<PathBuf>, opts: PanelOptions) -> Self {
        let root = root.into();
        let root = root.canonicalize().unwrap_or(root);
        let opts_bookmarks = opts.bookmarks.clone();
        let all: Vec<ui::keys::Action> =
            keys::ACTIONS.iter().cloned().chain(opts.actions.iter().cloned()).collect();
        let keymap = ui::keys::Keymap::new(&opts.leader, &all, keys::GROUPS);
        let mut p = Panel {
            root,
            opts,
            expanded: BTreeSet::new(),
            rows: Vec::new(),
            cursor: 0,
            offset: 0,
            page: 10,
            git: None,
            mode: Mode::Normal,
            marked: BTreeSet::new(),
            clip: None,
            message: None,
            bookmarks: Bookmarks::load(opts_bookmarks),
            keymap,
            dir_sizes: std::collections::HashMap::new(),
            sizes_shown: Vec::new(),
            job: None,
            pending: None,
            show_ignored: false,
            redraw: false,
        };
        p.refresh();
        p
    }

    /// The folder the tree starts from.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reread the directories, the git status and the folder sizes.
    pub fn refresh(&mut self) {
        self.dir_sizes.clear();
        self.rebuild();
    }

    /// Reread the directories and the git status.
    fn rebuild(&mut self) {
        self.git = if self.opts.git {
            Repo::discover(&self.root).and_then(|r| r.status_deep().ok().map(|s| (r, s)))
        } else {
            None
        };
        let keep = self.rows.get(self.cursor).map(|r| r.path.clone());
        self.rows.clear();
        let root = self.root.clone();
        self.build(&root, 0);
        if let Some(path) = keep
            && let Some(i) = self.rows.iter().position(|r| r.path == path)
        {
            self.cursor = i;
        }
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
    }

    /// The sorted, filtered entries of one folder: folders first, then by name, honoring the ignore
    /// and hidden-file settings.
    fn children(&self, dir: &Path) -> Vec<Row> {
        // With ignored files shown, the entries the ignore files would hide are marked `ignored`.
        let kept: Option<BTreeSet<std::ffi::OsString>> = self.show_ignored.then(|| {
            WalkBuilder::new(dir)
                .max_depth(Some(1))
                .hidden(!self.opts.hidden)
                .filter_entry(|e| e.file_name() != ".git")
                .build()
                .flatten()
                .filter(|e| e.depth() == 1)
                .map(|e| e.file_name().to_owned())
                .collect()
        });
        let walk = WalkBuilder::new(dir)
            .max_depth(Some(1))
            .hidden(!self.opts.hidden)
            .git_ignore(!self.show_ignored)
            .git_global(!self.show_ignored)
            .git_exclude(!self.show_ignored)
            .ignore(!self.show_ignored)
            .parents(!self.show_ignored)
            .filter_entry(|e| e.file_name() != ".git")
            .build();
        let mut out: Vec<Row> = walk
            .flatten()
            .filter(|e| e.depth() == 1)
            .map(|e| Row {
                ignored: kept.as_ref().is_some_and(|k| !k.contains(e.file_name())),
                name: e.file_name().to_string_lossy().into_owned(),
                is_dir: e.file_type().is_some_and(|t| t.is_dir()),
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                modified: e.metadata().ok().and_then(|m| m.modified().ok()),
                mode: e.metadata().map(|m| meta::mode_of(&m)).unwrap_or(0),
                path: e.into_path(),
                depth: 0,
            })
            .collect();
        out.sort_by(|a, b| {
            b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        out
    }

    /// Append the rows of `dir` and, for each expanded folder, its children,
    /// so the list is the tree flattened.
    fn build(&mut self, dir: &Path, depth: u16) {
        let rows = self.children(dir);
        for mut row in rows {
            row.depth = depth;
            let open = row.is_dir && self.expanded.contains(&row.path);
            let path = row.path.clone();
            self.rows.push(row);
            if open {
                self.build(&path, depth + 1);
            }
        }
    }

    /// Expand the folders down to `path` and put the cursor on it.
    pub fn reveal(&mut self, path: &Path) {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let Ok(rel_path) = path.strip_prefix(&self.root) else { return };
        let mut dir = self.root.clone();
        for part in rel_path.parent().into_iter().flat_map(|p| p.components()) {
            dir.push(part);
            self.expanded.insert(dir.clone());
        }
        self.refresh();
        if let Some(i) = self.rows.iter().position(|r| r.path == path) {
            self.cursor = i;
        }
    }

    /// Make `dir` the root of the tree (cd).
    pub fn set_root(&mut self, dir: PathBuf) {
        let dir = dir.canonicalize().unwrap_or(dir);
        self.root = dir;
        self.expanded.clear();
        self.marked.clear();
        self.cursor = 0;
        self.offset = 0;
        self.rebuild();
    }

    /// Show a message (or error) in the footer until the next key.
    pub fn notify(&mut self, msg: impl Into<String>, error: bool) {
        self.show_message(msg, error);
    }

    /// Forget marks and cancel nothing else: call after the host acted on them.
    pub fn clear_marks(&mut self) {
        self.marked.clear();
    }

    /// The marked rows, or the one under the cursor.
    pub fn targets(&self) -> Vec<PathBuf> {
        if self.marked.is_empty() {
            self.current().map(|p| vec![p.to_path_buf()]).unwrap_or_default()
        } else {
            self.marked.iter().cloned().collect()
        }
    }

    /// The folder rows that enclose row `at`, outermost first (what stays pinned above it).
    fn ancestors(&self, at: usize) -> Vec<usize> {
        let Some(row) = self.rows.get(at) else { return Vec::new() };
        let mut need = row.depth;
        let mut found = Vec::new();
        for i in (0..at).rev() {
            if need == 0 {
                break;
            }
            if self.rows[i].is_dir && self.rows[i].depth < need {
                found.push(i);
                need = self.rows[i].depth;
            }
        }
        found.reverse();
        found
    }

    /// The pinned folder rows when `offset` is the first scrolling row; at least one row scrolls.
    fn pinned(&self, offset: usize) -> Vec<usize> {
        let mut p = self.ancestors(offset);
        let keep = self.page.saturating_sub(1);
        if p.len() > keep {
            p.drain(..p.len() - keep);
        }
        p
    }

    /// The rows drawn, top to bottom: the pinned folders, then the rows from `offset`.
    fn visible(&self) -> Vec<usize> {
        let pinned = self.pinned(self.offset);
        let take = self.page.saturating_sub(pinned.len());
        pinned.into_iter().chain(self.offset..(self.offset + take).min(self.rows.len())).collect()
    }

    /// The width the pane needs to show the rows on screen without cutting a name off: the longest one
    /// (guides, icon, name, the git sign) and the pane's own border and padding.
    pub fn wanted_width(&self, theme: &Theme) -> u16 {
        let chrome = 100 - Pane::new(theme).inner(Rect::new(0, 0, 100, 10)).width;
        let longest = self
            .visible()
            .into_iter()
            .filter_map(|i| self.rows.get(i))
            .map(|r| usize::from(r.depth) * 2 + 2 + r.name.chars().count() + 5)
            .max()
            .unwrap_or(0);
        (longest as u16).saturating_add(chrome)
    }

    /// The scope the cursor is in, for the highlighted indent guide.
    fn active_scope(&self) -> Option<ui::tree::Scope> {
        let depths: Vec<usize> = self.rows.iter().map(|r| usize::from(r.depth)).collect();
        ui::tree::active_scope(&depths, self.cursor)
    }

    /// Path under the cursor.
    pub fn current(&self) -> Option<&Path> {
        self.rows.get(self.cursor).map(|r| r.path.as_path())
    }

    /// Move the cursor by `delta` rows, staying inside the list.
    fn move_by(&mut self, delta: isize) {
        let last = self.rows.len().saturating_sub(1) as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, last.max(0)) as usize;
    }

    /// Open or close the folder under the cursor (`open` forces a state, `None` flips it).
    fn toggle(&mut self, open: Option<bool>) {
        let Some(row) = self.rows.get(self.cursor) else { return };
        if !row.is_dir {
            return;
        }
        let path = row.path.clone();
        let is_open = self.expanded.contains(&path);
        match (open, is_open) {
            (Some(true), true) | (Some(false), false) => return,
            _ if is_open => self.expanded.remove(&path),
            _ => self.expanded.insert(path),
        };
        self.rebuild();
    }

    /// `h`: collapse an open folder, otherwise go to the parent folder's row.
    fn left(&mut self) {
        let Some(row) = self.rows.get(self.cursor) else { return };
        if row.is_dir && self.expanded.contains(&row.path) {
            self.toggle(Some(false));
        } else if let Some(parent) = row.path.parent()
            && let Some(i) = self.rows.iter().position(|r| r.path == parent)
        {
            self.cursor = i;
        }
    }

    /// Where `a` and `p` put things: the folder under the cursor, else its parent.
    fn target_dir(&self) -> PathBuf {
        match self.rows.get(self.cursor) {
            Some(r) if r.is_dir => r.path.clone(),
            Some(r) => r.path.parent().map(Path::to_path_buf).unwrap_or_else(|| self.root.clone()),
            None => self.root.clone(),
        }
    }

    /// Drops the footer message, as the next key press does.
    pub fn clear_message(&mut self) {
        self.message = None;
    }

    /// Set the footer message; `error` makes it show as a failure.
    fn show_message(&mut self, msg: impl Into<String>, error: bool) {
        self.message = Some((msg.into(), error));
    }

    /// Whether a prompt or a confirmation is open (keys belong to it, not to the host).
    pub fn is_busy(&self) -> bool {
        !matches!(self.mode, Mode::Normal)
    }

    /// Whether a prompt is taking typed text (so keys like space are text, not commands).
    pub fn is_typing(&self) -> bool {
        matches!(self.mode, Mode::Prompt(..))
    }

    /// Feed one terminal event to the panel and return what the host should do about it. Also
    /// collects the end of a finished background paste.
    pub fn handle_event(&mut self, ev: &Event) -> Outcome {
        self.poll_job();
        let action = self.handle_key_event(ev);
        if action == Outcome::None { self.pending.take().unwrap_or(Outcome::None) } else { action }
    }

    /// Route a key to the open prompt, confirmation or bookmark wait, else to the normal keymap.
    fn handle_key_event(&mut self, ev: &Event) -> Outcome {
        let Event::Key(key @ KeyEvent { kind, .. }) = ev else { return Outcome::None };
        if *kind == KeyEventKind::Release {
            return Outcome::None;
        }
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Prompt(kind, input) => return self.prompt_key(kind, input, ev, key),
            Mode::Confirm(kind, targets) => return self.confirm_key(kind, targets, key),
            Mode::Bookmark(op) => return self.bookmark_key(op, key),
            Mode::Normal => {}
        }
        self.message = None;
        self.normal_key(key)
    }

    /// While a paste runs, Esc cancels it and the delete keys are refused: the action to return
    /// for such a key, `None` for any other.
    fn job_key(&mut self, key: &KeyEvent) -> Option<Outcome> {
        let job = self.job.as_ref()?;
        if key.code == KeyCode::Esc {
            job.progress.cancel.store(true, Ordering::Relaxed);
            self.show_message("cancelling the paste", false);
            return Some(Outcome::None);
        }
        if matches!(key.code, KeyCode::Char('d' | 'D')) {
            self.show_message("busy: a paste is running (Esc cancels it)", true);
            return Some(Outcome::None);
        }
        None
    }

    /// Whether this event is the panel's own even though the host owns the keymap: a prompt, a
    /// question or a bookmark wait is open, or a paste runs and the key cancels or refuses it.
    pub fn captures_input(&self, ev: &Event) -> bool {
        if self.is_busy() {
            return true;
        }
        let Event::Key(key) = ev else { return false };
        self.job.is_some() && matches!(key.code, KeyCode::Esc | KeyCode::Char('d' | 'D'))
    }

    /// Feeds an event the panel claimed (see `captures`) when the host owns the keymap, and returns
    /// what the host should do about it.
    pub fn handle_captured(&mut self, ev: &Event) -> Outcome {
        self.poll_job();
        let Event::Key(key @ KeyEvent { kind, .. }) = ev else { return Outcome::None };
        if *kind == KeyEventKind::Release {
            return Outcome::None;
        }
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Prompt(kind, input) => self.prompt_key(kind, input, ev, key),
            Mode::Confirm(kind, targets) => self.confirm_key(kind, targets, key),
            Mode::Bookmark(op) => self.bookmark_key(op, key),
            Mode::Normal => self.job_key(key).unwrap_or(Outcome::None),
        }
    }

    /// A key in the normal mode: cancels or refuses things while a paste runs,
    /// else the keymap decides.
    fn normal_key(&mut self, key: &KeyEvent) -> Outcome {
        if let Some(action) = self.job_key(key) {
            return action;
        }
        match self.keymap.feed(key) {
            ui::keys::Feed::Run(id) => self.run(id),
            ui::keys::Feed::NoBinding => {
                let seq = self.keymap.rejected().to_string();
                self.show_message(format!("{seq} is not bound"), true);
                Outcome::None
            }
            ui::keys::Feed::Pending | ui::keys::Feed::Cancelled | ui::keys::Feed::Unbound => {
                Outcome::None
            }
        }
    }

    /// The which-key menu while a leader sequence is under way.
    pub fn menu(&self) -> Option<ui::keys::Menu> {
        self.keymap.menu()
    }

    /// The actions this panel offers (its own and the host's), for a palette or help.
    pub fn actions(&self) -> &[ui::keys::Action] {
        self.keymap.actions()
    }

    /// Every action with its keys, for the palette and the help.
    pub fn keys_rows(&self) -> Vec<ui::keys::Row> {
        self.keymap.rows()
    }

    /// The keys bound to an action, as shown.
    pub fn keys_of(&self, id: &str) -> String {
        self.keymap.keys_of(id)
    }

    /// Run an action by id (what a key, the leader menu or a palette resolves to).
    pub fn run(&mut self, id: &'static str) -> Outcome {
        if !keys::is_own_action(id) {
            return Outcome::Command(id);
        }
        match id.split('.').next().unwrap_or(id) {
            "nav" | "tree" => self.run_nav_action(id),
            "item" => self.run_item_action(id),
            "search" => self.run_search_action(id),
            "view" => self.run_view_action(id),
            "root" => self.run_root_action(id),
            "bookmark" => self.run_bookmark_action(id),
            _ => self.run_general_action(id),
        }
    }

    /// Runs one of the actions that belong to no domain: back, the palette, the help and quit.
    fn run_general_action(&mut self, id: &str) -> Outcome {
        match id {
            "back" => {
                if self.marked.is_empty() {
                    return Outcome::Close;
                }
                self.marked.clear();
            }
            "palette" => return Outcome::Palette,
            "help" => return Outcome::Help,
            "quit" => return Outcome::Close,
            _ => {}
        }
        Outcome::None
    }

    /// A key while a text prompt (new, rename, open with) is open: Enter submits, Esc drops it.
    fn prompt_key(
        &mut self,
        kind: PromptKind,
        mut input: Input,
        ev: &Event,
        key: &KeyEvent,
    ) -> Outcome {
        match key.code {
            KeyCode::Esc => Outcome::None,
            KeyCode::Enter => {
                let value = input.value().to_string();
                match kind {
                    PromptKind::Create(dir) => self.create(&dir, &value),
                    PromptKind::Rename(from) => self.rename(&from, &value),
                    PromptKind::OpenWith(path) => Outcome::OpenWith(path, value),
                }
            }
            _ => {
                input.handle_event(ev);
                self.mode = Mode::Prompt(kind, input);
                Outcome::None
            }
        }
    }

    /// The key typed after a bookmark command: it is the bookmark's name, and the open operation
    /// saves, jumps to or deletes it.
    fn bookmark_key(&mut self, op: BookmarkOp, key: &KeyEvent) -> Outcome {
        let KeyCode::Char(c) = key.code else { return Outcome::None };
        match op {
            BookmarkOp::Set => {
                let dir = self.target_dir();
                match self.bookmarks.set(c, dir.clone()) {
                    Ok(()) => self.show_message(format!("bookmark {c}: {}", dir.display()), false),
                    Err(e) => self.show_message(format!("bookmark not saved: {e}"), true),
                }
            }
            BookmarkOp::Jump => match self.bookmarks.get(c).map(Path::to_path_buf) {
                Some(dir) if dir.is_dir() => self.set_root(dir),
                Some(dir) => self.show_message(format!("{} is gone", dir.display()), true),
                None => self.show_message(format!("no bookmark {c}"), true),
            },
            BookmarkOp::Delete => match self.bookmarks.remove(c) {
                Ok(true) => self.show_message(format!("removed bookmark {c}"), false),
                Ok(false) => self.show_message(format!("no bookmark {c}"), true),
                Err(e) => self.show_message(format!("bookmark not saved: {e}"), true),
            },
        }
        Outcome::None
    }

    /// A key at a delete confirmation: the confirm key or Enter deletes, anything else cancels.
    fn confirm_key(&mut self, kind: ConfirmKind, targets: Vec<PathBuf>, key: &KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Char(c) if c == kind.confirm_key() => self.delete(kind, targets),
            KeyCode::Enter => self.delete(kind, targets),
            _ => Outcome::None,
        }
    }

    /// Create the file or folder typed in the new prompt and move the cursor onto it.
    fn create(&mut self, dir: &Path, name: &str) -> Outcome {
        match ops::create(dir, name) {
            Ok((path, is_dir)) => {
                self.show_message(format!("created {}", name.trim_end_matches('/')), false);
                self.reveal(&path);
                Outcome::Created(path, is_dir)
            }
            Err(e) => {
                self.show_message(e.to_string(), true);
                Outcome::None
            }
        }
    }

    /// Rename `from` to the typed name, keeping open folders open and
    /// putting the cursor on the result.
    fn rename(&mut self, from: &Path, name: &str) -> Outcome {
        match ops::rename(from, name) {
            Ok(to) if to == from => Outcome::None,
            Ok(to) => {
                self.remap_expanded(from, &to);
                self.marked.clear();
                self.show_message(format!("renamed to {name}"), false);
                self.reveal(&to);
                Outcome::Moved(vec![(from.to_path_buf(), to)])
            }
            Err(e) => {
                self.show_message(e.to_string(), true);
                Outcome::None
            }
        }
    }

    /// Keep folders open across a rename or move.
    fn remap_expanded(&mut self, from: &Path, to: &Path) {
        let moved: Vec<PathBuf> =
            self.expanded.iter().filter(|p| p.starts_with(from)).cloned().collect();
        for old in moved {
            self.expanded.remove(&old);
            if let Ok(rest) = old.strip_prefix(from) {
                self.expanded.insert(to.join(rest));
            }
        }
    }

    /// Trash or permanently delete the confirmed targets, then reread the tree either way.
    fn delete(&mut self, kind: ConfirmKind, targets: Vec<PathBuf>) -> Outcome {
        let result = match kind {
            ConfirmKind::Trash => ops::trash(&targets),
            ConfirmKind::Delete => {
                targets.iter().try_for_each(|p| ops::remove(p)).map_err(|e| e.to_string())
            }
        };
        match result {
            Ok(()) => {
                let n = targets.len();
                self.show_message(
                    format!(
                        "{} {n} item{}",
                        if kind == ConfirmKind::Trash { "trashed" } else { "deleted" },
                        if n == 1 { "" } else { "s" }
                    ),
                    false,
                );
                self.marked.clear();
                self.refresh();
                Outcome::Deleted(targets)
            }
            Err(e) => {
                self.show_message(e, true);
                self.refresh();
                Outcome::None
            }
        }
    }
}

/// The rows are what the shared movement walks.
impl ui::app::Navigable for Panel {
    fn move_by(&mut self, delta: isize) {
        Panel::move_by(self, delta);
    }

    fn page_row_count(&self) -> usize {
        self.page
    }

    fn move_to_top(&mut self) {
        self.cursor = 0;
    }

    fn move_to_bottom(&mut self) {
        self.cursor = self.rows.len().saturating_sub(1);
    }
}
