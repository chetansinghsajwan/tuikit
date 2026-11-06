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
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
use ui::{Pane, Theme};

mod paste;
mod render;
mod types;
use paste::*;
use types::*;
pub use types::{Action, Linemode, PanelOptions, Search};

#[cfg(test)]
mod tests;

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
    pending: Option<Action>,
    /// Something finished by itself since the last `tick`.
    redraw: bool,
    /// Show the entries that .gitignore and the other ignore files hide (toggle with `I`).
    show_ignored: bool,
    /// How many entries of the shown folders the ignore files hide now (the title says so).
    ignored_hidden: usize,
}

impl Panel {
    pub fn new(root: impl Into<PathBuf>, opts: PanelOptions) -> Self {
        let root = root.into();
        let root = root.canonicalize().unwrap_or(root);
        let opts_bookmarks = opts.bookmarks.clone();
        let all: Vec<ui::keys::Action> = keys::ACTIONS.iter().cloned().chain(opts.actions.iter().cloned()).collect();
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
            ignored_hidden: 0,
            redraw: false,
        };
        p.refresh();
        p
    }

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
        self.git = if self.opts.git { Repo::discover(&self.root).and_then(|r| r.status().ok().map(|s| (r, s))) } else { None };
        let keep = self.rows.get(self.cursor).map(|r| r.path.clone());
        self.rows.clear();
        self.ignored_hidden = 0;
        let root = self.root.clone();
        self.build(&root, 0);
        if let Some(path) = keep {
            if let Some(i) = self.rows.iter().position(|r| r.path == path) {
                self.cursor = i;
            }
        }
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
    }

    fn children(&self, dir: &Path) -> Vec<Row> {
        let walk = WalkBuilder::new(dir).max_depth(Some(1)).hidden(!self.opts.hidden).git_ignore(!self.show_ignored).git_global(!self.show_ignored).git_exclude(!self.show_ignored).ignore(!self.show_ignored).parents(!self.show_ignored).filter_entry(|e| e.file_name() != ".git").build();
        let mut out: Vec<Row> = walk
            .flatten()
            .filter(|e| e.depth() == 1)
            .map(|e| Row {
                name: e.file_name().to_string_lossy().into_owned(),
                is_dir: e.file_type().is_some_and(|t| t.is_dir()),
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                modified: e.metadata().ok().and_then(|m| m.modified().ok()),
                mode: e.metadata().map(|m| meta::mode_of(&m)).unwrap_or(0),
                path: e.into_path(),
                depth: 0,
            })
            .collect();
        out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        out
    }

    fn build(&mut self, dir: &Path, depth: u16) {
        let rows = self.children(dir);
        if !self.show_ignored {
            let all = std::fs::read_dir(dir).map(|d| d.flatten().filter(|e| e.file_name() != ".git" && (self.opts.hidden || !e.file_name().to_string_lossy().starts_with('.'))).count()).unwrap_or(0);
            self.ignored_hidden += all.saturating_sub(rows.len());
        }
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
        let Ok(rela) = path.strip_prefix(&self.root) else { return };
        let mut dir = self.root.clone();
        for part in rela.parent().into_iter().flat_map(|p| p.components()) {
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
        self.say(msg, error);
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
        let longest = self.visible().into_iter().filter_map(|i| self.rows.get(i)).map(|r| usize::from(r.depth) * 2 + 2 + r.name.chars().count() + 5).max().unwrap_or(0);
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

    fn move_by(&mut self, delta: isize) {
        let last = self.rows.len().saturating_sub(1) as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, last.max(0)) as usize;
    }

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
        } else if let Some(parent) = row.path.parent() {
            if let Some(i) = self.rows.iter().position(|r| r.path == parent) {
                self.cursor = i;
            }
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

    fn say(&mut self, msg: impl Into<String>, error: bool) {
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

    pub fn handle_event(&mut self, ev: &Event) -> Action {
        self.poll_job();
        let action = self.handle_key_event(ev);
        if action == Action::None { self.pending.take().unwrap_or(Action::None) } else { action }
    }

    fn handle_key_event(&mut self, ev: &Event) -> Action {
        let Event::Key(key @ KeyEvent { kind, .. }) = ev else { return Action::None };
        if *kind == KeyEventKind::Release {
            return Action::None;
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

    fn normal_key(&mut self, key: &KeyEvent) -> Action {
        if let Some(job) = &self.job {
            if key.code == KeyCode::Esc {
                job.progress.cancel.store(true, Ordering::Relaxed);
                self.say("cancelling the paste", false);
                return Action::None;
            }
            if matches!(key.code, KeyCode::Char('d' | 'D')) {
                self.say("busy: a paste is running (Esc cancels it)", true);
                return Action::None;
            }
        }
        match self.keymap.feed(key) {
            ui::keys::Feed::Run(id) => self.run(id),
            ui::keys::Feed::NoBinding => {
                let seq = self.keymap.rejected().to_string();
                self.say(format!("{seq} is not bound"), true);
                Action::None
            }
            ui::keys::Feed::Pending | ui::keys::Feed::Cancelled | ui::keys::Feed::Unbound => Action::None,
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
    pub fn run(&mut self, id: &'static str) -> Action {
        if !keys::owns(id) {
            return Action::Command(id);
        }
        match id {
            "nav.up" => self.move_by(-1),
            "nav.down" => self.move_by(1),
            "nav.up5" => self.move_by(-5),
            "nav.down5" => self.move_by(5),
            "nav.page_up" => self.move_by(-(self.page as isize)),
            "nav.page_down" => self.move_by(self.page as isize),
            "nav.top" => self.cursor = 0,
            "nav.bottom" => self.cursor = self.rows.len().saturating_sub(1),
            "tree.out" => self.left(),
            "item.open" => {
                if let Some(row) = self.rows.get(self.cursor) {
                    if row.is_dir {
                        self.toggle(Some(true));
                    } else {
                        return Action::Open(row.path.clone());
                    }
                }
            }
            "item.mark" => {
                if let Some(path) = self.current().map(Path::to_path_buf) {
                    if !self.marked.remove(&path) {
                        self.marked.insert(path);
                    }
                    self.move_by(1);
                }
            }
            "item.new" => self.mode = Mode::Prompt(PromptKind::Create(self.target_dir()), Input::default()),
            "item.rename" => {
                if let Some(row) = self.rows.get(self.cursor) {
                    self.mode = Mode::Prompt(PromptKind::Rename(row.path.clone()), Input::new(row.name.clone()));
                }
            }
            "item.delete" | "item.delete_forever" => {
                let targets = self.targets();
                if !targets.is_empty() {
                    self.mode = Mode::Confirm(if id == "item.delete" { ConfirmKind::Trash } else { ConfirmKind::Delete }, targets);
                }
            }
            "item.yank" | "item.cut" => {
                let paths = self.targets();
                if !paths.is_empty() {
                    let (n, cut) = (paths.len(), id == "item.cut");
                    self.clip = Some(Clip { paths, cut });
                    self.marked.clear();
                    self.say(format!("{} {n} item{}", if cut { "cut" } else { "yanked" }, if n == 1 { "" } else { "s" }), false);
                }
            }
            "item.paste" => return self.paste(),
            "item.open_with" => {
                if let Some(row) = self.rows.get(self.cursor) {
                    self.mode = Mode::Prompt(PromptKind::OpenWith(row.path.clone()), Input::default());
                }
            }
            "item.bulk_rename" => {
                let t = self.targets();
                if !t.is_empty() {
                    return Action::BulkRename(t);
                }
            }
            "search.files" => return Action::Search(Search::Files),
            "search.here" => return Action::Search(Search::Here(self.target_dir())),
            "search.grep" => return Action::Search(Search::Grep),
            "view.ignored" => {
                self.show_ignored = !self.show_ignored;
                self.rebuild();
            }
            "view.hidden" => {
                self.opts.hidden = !self.opts.hidden;
                self.rebuild();
            }
            "view.raw" => {
                pv::set_raw_tables(!pv::raw_tables());
                self.say(if pv::raw_tables() { "tables show as text" } else { "tables show as tables" }, false);
            }
            "view.columns" => {
                self.opts.linemode = self.opts.linemode.next();
                self.say(self.opts.linemode.label(), false);
            }
            "view.refresh" => self.refresh(),
            "root.into" => {
                if let Some(row) = self.rows.get(self.cursor).filter(|r| r.is_dir) {
                    let dir = row.path.clone();
                    self.set_root(dir);
                }
            }
            "root.focus" => {
                // The folder the cursor row sits in becomes the root: its siblings only, no other branches.
                if let Some(path) = self.current().map(Path::to_path_buf) {
                    if let Some(dir) = path.parent().map(Path::to_path_buf) {
                        self.set_root(dir);
                        self.reveal(&path);
                    }
                }
            }
            "root.up" => {
                if let Some(parent) = self.root.parent().map(Path::to_path_buf) {
                    let old = self.root.clone();
                    self.set_root(parent);
                    self.reveal(&old);
                }
            }
            "bookmark.set" => self.mode = Mode::Bookmark(BookmarkOp::Set),
            "bookmark.jump" => self.mode = Mode::Bookmark(BookmarkOp::Jump),
            "bookmark.delete" => self.mode = Mode::Bookmark(BookmarkOp::Delete),
            "back" => {
                if self.marked.is_empty() {
                    return Action::Close;
                }
                self.marked.clear();
            }
            "palette" => return Action::Palette,
            "help" => return Action::Help,
            "quit" => return Action::Close,
            _ => {}
        }
        Action::None
    }

    fn prompt_key(&mut self, kind: PromptKind, mut input: Input, ev: &Event, key: &KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::None,
            KeyCode::Enter => {
                let value = input.value().to_string();
                match kind {
                    PromptKind::Create(dir) => self.create(&dir, &value),
                    PromptKind::Rename(from) => self.rename(&from, &value),
                    PromptKind::OpenWith(path) => Action::OpenWith(path, value),
                }
            }
            _ => {
                input.handle_event(ev);
                self.mode = Mode::Prompt(kind, input);
                Action::None
            }
        }
    }

    fn bookmark_key(&mut self, op: BookmarkOp, key: &KeyEvent) -> Action {
        let KeyCode::Char(c) = key.code else { return Action::None };
        match op {
            BookmarkOp::Set => {
                let dir = self.target_dir();
                match self.bookmarks.set(c, dir.clone()) {
                    Ok(()) => self.say(format!("bookmark {c}: {}", dir.display()), false),
                    Err(e) => self.say(format!("bookmark not saved: {e}"), true),
                }
            }
            BookmarkOp::Jump => match self.bookmarks.get(c).map(Path::to_path_buf) {
                Some(dir) if dir.is_dir() => self.set_root(dir),
                Some(dir) => self.say(format!("{} is gone", dir.display()), true),
                None => self.say(format!("no bookmark {c}"), true),
            },
            BookmarkOp::Delete => match self.bookmarks.remove(c) {
                Ok(true) => self.say(format!("removed bookmark {c}"), false),
                Ok(false) => self.say(format!("no bookmark {c}"), true),
                Err(e) => self.say(format!("bookmark not saved: {e}"), true),
            },
        }
        Action::None
    }

    fn confirm_key(&mut self, kind: ConfirmKind, targets: Vec<PathBuf>, key: &KeyEvent) -> Action {
        match key.code {
            KeyCode::Char(c) if c == kind.confirm_key() => self.delete(kind, targets),
            KeyCode::Enter => self.delete(kind, targets),
            _ => Action::None,
        }
    }

    fn create(&mut self, dir: &Path, name: &str) -> Action {
        match ops::create(dir, name) {
            Ok((path, is_dir)) => {
                self.say(format!("created {}", name.trim_end_matches('/')), false);
                self.reveal(&path);
                Action::Created(path, is_dir)
            }
            Err(e) => {
                self.say(e.to_string(), true);
                Action::None
            }
        }
    }

    fn rename(&mut self, from: &Path, name: &str) -> Action {
        match ops::rename(from, name) {
            Ok(to) if to == from => Action::None,
            Ok(to) => {
                self.remap_expanded(from, &to);
                self.marked.clear();
                self.say(format!("renamed to {name}"), false);
                self.reveal(&to);
                Action::Moved(vec![(from.to_path_buf(), to)])
            }
            Err(e) => {
                self.say(e.to_string(), true);
                Action::None
            }
        }
    }

    /// Keep folders open across a rename or move.
    fn remap_expanded(&mut self, from: &Path, to: &Path) {
        let moved: Vec<PathBuf> = self.expanded.iter().filter(|p| p.starts_with(from)).cloned().collect();
        for old in moved {
            self.expanded.remove(&old);
            if let Ok(rest) = old.strip_prefix(from) {
                self.expanded.insert(to.join(rest));
            }
        }
    }

    fn delete(&mut self, kind: ConfirmKind, targets: Vec<PathBuf>) -> Action {
        let result = match kind {
            ConfirmKind::Trash => ops::trash(&targets),
            ConfirmKind::Delete => targets.iter().try_for_each(|p| ops::remove(p)).map_err(|e| e.to_string()),
        };
        match result {
            Ok(()) => {
                let n = targets.len();
                self.say(format!("{} {n} item{}", if kind == ConfirmKind::Trash { "trashed" } else { "deleted" }, if n == 1 { "" } else { "s" }), false);
                self.marked.clear();
                self.refresh();
                Action::Deleted(targets)
            }
            Err(e) => {
                self.say(e, true);
                self.refresh();
                Action::None
            }
        }
    }

}
