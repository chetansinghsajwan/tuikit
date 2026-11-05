use git_core::{CommitInfo, Operation, RefInfo, RefKind, Repo, ResetMode, StashInfo};
use git_panel::{Action as PanelAction, Panel};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::path::{Path, PathBuf};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;
use ui::keys::{Feed, Keymap};
use ui::{Pane, Theme};

/// How many commits the Commits tab loads.
const TAB_LOG_LIMIT: usize = 3000;
/// Most cells (two per lane) the graph column may take.
const GRAPH_MAX: usize = 24;
/// Most cells the refs column of the Commits table may take.
const REFS_COL: usize = 24;

/// What the host should do after an event.
#[derive(Debug, PartialEq, Eq)]
pub enum Quit {
    No,
    Yes,
    /// Open this file in the editor (the host suspends the screen), then call `edited`.
    Edit(PathBuf),
    /// The user asked for the command palette: the host runs `ui::keys::choose` over
    /// `App::keys_rows` and passes the pick to `App::run_palette`.
    Palette,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Files,
    Branches,
    Commits,
    Stashes,
}

/// The panes of each tab, in focus order.
const STATUS_PANES: [Focus; 1] = [Focus::Files];
const COMMITS_PANES: [Focus; 1] = [Focus::Commits];
const REFS_PANES: [Focus; 2] = [Focus::Branches, Focus::Stashes];

enum PromptKind {
    /// Search the commits.
    Search,
    NewBranch,
    NewStash,
    /// Rename this local branch.
    RenameBranch(String),
    /// Rename this tag, this remote branch (`origin/topic`), this remote, or every branch or tag under a folder.
    RenameTag(String),
    RenameRemoteBranch(String),
    RenameRemote(String),
    RenameFolder { kind: RefKind, prefix: String },
    /// A branch at this commit.
    BranchAt(String),
    /// A tag at this commit.
    TagAt(String),
    /// A new message for the last commit.
    Reword(String),
    /// Check out a branch, tag, remote branch or commit by name.
    SwitchTo,
    /// Amend the last commit with what is staged, with this message (the old one is the value).
    Amend(String),
}

enum Confirm {
    DeleteBranch { name: String, force: bool },
    Merge(String),
    DropStash(usize),
    Rebase(String),
    Revert(String),
    Reset { oid: String, mode: ResetMode },
    ForcePush,
    /// Delete a tag.
    DeleteTag(String),
    /// Delete a branch on its remote (`origin/topic`).
    DeleteRemote(String),
}

/// A git command talking to a remote, running on its own thread.
struct Job {
    label: String,
    done: String,
    handle: std::thread::JoinHandle<Result<(), git_core::Error>>,
}

/// The tabs: the changed files, the full commit graph, and the refs (branches, tags, stashes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Status,
    Commits,
    Refs,
}

enum Mode {
    Normal,
    Prompt(PromptKind, Input),
    Confirm(Confirm),
    /// The filter popup.
    Popup(ui::multi::Multi),
}

/// A row of the Refs tree: a section (Branches, Remotes, Tags), a folder (`feature/`, `origin/`) or a ref.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RefRow {
    Section { key: String, label: &'static str, count: usize },
    Folder { key: String, name: String, depth: usize, count: usize },
    /// `index` into `branches`; `name` is the last path part.
    Item { index: usize, name: String, depth: usize },
}

/// Which columns of the Commits table show (`v` menu).
#[derive(Clone, Copy)]
struct Columns {
    hash: bool,
    author: bool,
    /// Authors as full names instead of two initials.
    author_name: bool,
    refs: bool,
    subject: bool,
    rel_time: bool,
    abs_time: bool,
}

impl Default for Columns {
    fn default() -> Self {
        Columns { hash: true, author: true, author_name: false, refs: true, subject: true, rel_time: true, abs_time: false }
    }
}

/// The files of one commit, as a folder tree (Commits tab, `l` on a commit); the diff of the one under the
/// cursor shows beside it.
struct CommitFiles {
    oid: String,
    short: String,
    subject: String,
    changes: Vec<git_core::FileChange>,
    rows: Vec<FileRow>,
    cursor: usize,
    offset: usize,
    folded: std::collections::HashSet<String>,
}

enum FileRow {
    Folder { path: String, label: String, depth: usize, count: usize, folded: bool },
    File { change: git_core::FileChange, depth: usize },
}

impl FileRow {
    fn path(&self) -> &str {
        match self {
            FileRow::Folder { path, .. } => path,
            FileRow::File { change, .. } => &change.path,
        }
    }
}

#[derive(Default)]
struct FileNode {
    dirs: std::collections::BTreeMap<String, FileNode>,
    files: Vec<git_core::FileChange>,
}

impl FileNode {
    fn count(&self) -> usize {
        self.files.len() + self.dirs.values().map(FileNode::count).sum::<usize>()
    }
}

impl CommitFiles {
    fn new(oid: String, short: String, subject: String, mut changes: Vec<git_core::FileChange>) -> CommitFiles {
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        let mut cf = CommitFiles { oid, short, subject, changes, rows: Vec::new(), cursor: 0, offset: 0, folded: Default::default() };
        cf.rebuild();
        cf
    }

    fn rebuild(&mut self) {
        let keep = self.rows.get(self.cursor).map(|r| r.path().to_string());
        let mut root = FileNode::default();
        for c in &self.changes {
            let mut node = &mut root;
            let mut parts: Vec<&str> = c.path.split('/').collect();
            parts.pop();
            for part in parts {
                node = node.dirs.entry(part.to_string()).or_default();
            }
            node.files.push(c.clone());
        }
        let mut rows = Vec::new();
        fn push(node: &FileNode, prefix: &str, depth: usize, folded: &std::collections::HashSet<String>, rows: &mut Vec<FileRow>) {
            for (name, child) in &node.dirs {
                let (mut label, mut child, mut path) = (name.clone(), child, if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") });
                // A folder with nothing but one folder in it joins it: `src/lib`.
                while child.files.is_empty() && child.dirs.len() == 1 {
                    let (n, c) = child.dirs.iter().next().expect("one folder");
                    label = format!("{label}/{n}");
                    path = format!("{path}/{n}");
                    child = c;
                }
                let is_folded = folded.contains(&path);
                rows.push(FileRow::Folder { path: path.clone(), label, depth, count: child.count(), folded: is_folded });
                if !is_folded {
                    push(child, &path, depth + 1, folded, rows);
                }
            }
            for change in &node.files {
                rows.push(FileRow::File { change: change.clone(), depth });
            }
        }
        push(&root, "", 0, &self.folded, &mut rows);
        self.rows = rows;
        self.cursor = keep.and_then(|p| self.rows.iter().position(|r| r.path() == p)).unwrap_or(self.cursor).min(self.rows.len().saturating_sub(1));
    }

    fn move_by(&mut self, delta: isize) {
        let last = self.rows.len().saturating_sub(1) as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, last.max(0)) as usize;
    }

    /// The path of the row under the cursor (a folder's covers every file below it).
    fn current_path(&self) -> Option<&str> {
        self.rows.get(self.cursor).map(FileRow::path)
    }
}

/// What narrows the commit list. The graph is drawn from what is left.
#[derive(Default, Clone)]
struct Filter {
    text: String,
    /// Commits by any of these authors.
    authors: Vec<String>,
    /// Only the history of these branches or tags (all of them together).
    branches: Vec<String>,
    /// Commits whose subject starts with any of these categories (`fix`, `feat`).
    categories: Vec<String>,
    /// As typed (`7d`) and as seconds since the epoch.
    since: Option<(String, i64)>,
}

impl Filter {
    fn active(&self) -> bool {
        !self.text.is_empty() || !self.authors.is_empty() || !self.branches.is_empty() || !self.categories.is_empty() || self.since.is_some()
    }

    /// The parts that are set, as `(what, value)` for the filter bar and the popup.
    fn parts(&self) -> Vec<(&'static str, String)> {
        let mut parts = Vec::new();
        if !self.branches.is_empty() {
            parts.push(("branch", self.branches.join(", ")));
        }
        if !self.authors.is_empty() {
            parts.push(("author", self.authors.join(", ")));
        }
        if !self.categories.is_empty() {
            parts.push(("category", self.categories.join(", ")));
        }
        if !self.text.is_empty() {
            parts.push(("message", self.text.clone()));
        }
        if let Some((s, _)) = &self.since {
            parts.push(("since", s.clone()));
        }
        parts
    }

    fn keeps(&self, c: &CommitInfo) -> bool {
        (self.text.is_empty() || c.message.to_lowercase().contains(&self.text.to_lowercase()))
            && (self.authors.is_empty() || self.authors.iter().any(|a| a.eq_ignore_ascii_case(&c.author)))
            && (self.categories.is_empty() || ui::subject::category_word(&c.subject).is_some_and(|w| self.categories.contains(&w)))
            && self.since.as_ref().is_none_or(|(_, t)| c.time >= *t)
    }
}

/// Seconds since the epoch for `7d`, `2w`, `3m` (30 days), `1y` or `2026-10-01`; `now` is the current time.
fn parse_since(text: &str, now: i64) -> Option<i64> {
    let text = text.trim();
    let (digits, unit) = text.split_at(text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len()));
    if let (Ok(n), Some(secs)) = (digits.parse::<i64>(), match unit { "d" => Some(86_400), "w" => Some(7 * 86_400), "m" => Some(30 * 86_400), "y" => Some(365 * 86_400), _ => None }) {
        return Some(now - n * secs);
    }
    // A date: days since 1970-01-01 by the civil-calendar formula.
    let mut it = text.split('-').map(|p| p.parse::<i64>());
    let (Some(Ok(y)), Some(Ok(m)), Some(Ok(d)), None) = (it.next(), it.next(), it.next(), it.next()) else { return None };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400)
}

#[derive(Default, Clone, Copy)]
struct ListState {
    cursor: usize,
    offset: usize,
}

struct Preview {
    key: (Focus, String, u16),
    lines: Vec<Line<'static>>,
    /// Where the files and hunks of the patch start (for the jump keys).
    anchors: pv::Anchors,
}

pub struct App {
    root: PathBuf,
    repo: Repo,
    theme: Theme,
    files: Panel,
    branches: Vec<RefInfo>,
    commits: Vec<CommitInfo>,
    stashes: Vec<StashInfo>,
    /// Cursor and scroll of the branches, commits and stashes panes.
    lists: [ListState; 3],
    focus: Focus,
    mode: Mode,
    message: Option<(String, bool)>,
    preview: Option<Preview>,
    pscroll: usize,
    /// The commit whose files were folded by hand (another commit starts with none).
    fold_rev: Option<String>,
    pheight: usize,
    /// The keymaps of the Branches, Commits and Stashes panes (the Files pane has its own).
    maps: [Keymap; 3],
    help: Option<ui::keys::Help>,
    /// A fetch, pull or push in progress.
    job: Option<Job>,
    /// The commit graph column in the Commits pane, and whether it covers every branch.
    graph: bool,
    all_branches: bool,
    graph_rows: Vec<git_core::GraphRow>,
    graph_w: usize,
    /// Width of the refs column in the Commits table.
    refs_w: usize,
    /// The width every ref of the widest commit needs together (the refs column is sized from it).
    refs_need: usize,
    /// The columns shown, and the width of the author names.
    cols: Columns,
    author_w: usize,
    /// The refs as the tree the Refs pane shows, and which sections and folders are folded.
    ref_rows: Vec<RefRow>,
    folded: std::collections::HashSet<String>,
    /// The refs (branches, tags) pointing at each commit, by hash, for the labels.
    labels: std::collections::HashMap<String, Vec<RefInfo>>,
    screen: Screen,
    /// The Commits tab shows the diff of the commit under the cursor beside the list.
    detail: bool,
    /// The commit marked to compare with: the preview shows its diff against the highlighted commit.
    compare: Option<String>,
    /// The preview shows the diff from the highlighted commit to the work tree.
    work: bool,
    /// The current commit search.
    search: Option<String>,
    filter: Filter,
    /// The files of the commit being explored, when there is one.
    commit_files: Option<CommitFiles>,
    /// The options offered while a filter prompt is open.
    suggest: ui::prompt::Suggest,
    /// Changes made outside this screen (a commit in another terminal, a saved file) reload it.
    watch: Option<ui::watch::Watch>,
}

impl App {
    /// The screen for the repository containing `dir`; `None` if there is none.
    pub fn new(dir: &Path) -> Option<App> {
        let repo = Repo::discover(dir)?;
        let root = repo.root().to_path_buf();
        let watch = ui::watch::Watch::new(&root, || {});
        
        let mut files = Panel::with_actions(dir, "space", crate::keys::GLOBAL.to_vec(), crate::keys::GROUPS)?;
        files.set_title("Files");
        let mut app = App {
            root,
            repo,
            theme: Theme::load(),
            files,
            branches: Vec::new(),
            commits: Vec::new(),
            stashes: Vec::new(),
            lists: [ListState::default(); 3],
            focus: Focus::Files,
            mode: Mode::Normal,
            message: None,
            preview: None,
            pscroll: 0,
            fold_rev: None,
            pheight: 10,
            maps: [0, 1, 2].map(|i| Keymap::new("space", &crate::keys::pane_actions(i), crate::keys::GROUPS)),
            help: None,
            job: None,
            graph: true,
            all_branches: true,
            graph_rows: Vec::new(),
            graph_w: 0,
            refs_w: 0,
            refs_need: 0,
            cols: Columns::default(),
            author_w: 0,
            ref_rows: Vec::new(),
            folded: Default::default(),
            labels: Default::default(),
            screen: Screen::Status,
            detail: false,
            compare: None,
            work: false,
            search: None,
            filter: Filter::default(),
            commit_files: None,
            suggest: Default::default(),
            watch,
        };
        app.reload_all();
        // The Refs pane starts on the checked-out branch.
        if let Some(i) = app.ref_rows.iter().position(|r| matches!(r, RefRow::Item { index, .. } if app.branches[*index].is_head)) {
            app.lists[0].cursor = i;
        }
        Some(app)
    }

    // ---- data -----------------------------------------------------------

    fn reload_history(&mut self) {
        if let Some(repo) = Repo::discover(&self.root) {
            self.repo = repo;
        }
        let mut refs = self.repo.refs().unwrap_or_default();
        // Local branches first (the checked-out one on top), then remotes, then tags.
        let rank = |r: &RefInfo| match r.kind {
            RefKind::Branch => 0,
            RefKind::Remote => 1,
            RefKind::Tag => 2,
        };
        refs.sort_by(|a, b| rank(a).cmp(&rank(b)).then(b.is_head.cmp(&a.is_head)).then(b.time.cmp(&a.time)));
        // The row the cursor is on, by identity (before the refs change under it).
        let keep = self.ref_row_id(self.lists[0].cursor);
        self.branches = refs;
        // The commits are only loaded while their tab is shown.
        let limit = TAB_LOG_LIMIT;
        let commits = if self.screen != Screen::Commits {
            Ok(Vec::new())
        } else if self.filter.branches.len() == 1 {
            self.repo.commits_from(&self.filter.branches[0], limit)
        } else if !self.filter.branches.is_empty() {
            // Several branches: their histories together, each commit once.
            let mut all: Vec<CommitInfo> = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for b in &self.filter.branches {
                for k in self.repo.commits_from(b, limit).unwrap_or_default() {
                    if seen.insert(k.oid.clone()) {
                        all.push(k);
                    }
                }
            }
            all.sort_by(|a, b| b.time.cmp(&a.time));
            all.truncate(limit);
            Ok(git_core::topo_order(all))
        } else if self.all_branches {
            self.repo.commits_all(limit)
        } else {
            self.repo.commits(limit)
        };
        let mut commits = commits.unwrap_or_default();
        if self.filter.active() {
            let filter = self.filter.clone();
            commits = git_core::simplify(commits, move |c| filter.keeps(c));
        }
        self.commits = commits;
        self.graph_rows = if self.graph { git_core::graph(&self.commits) } else { Vec::new() };
        self.graph_w = self.graph_rows.iter().map(|r| r.cells.len()).max().unwrap_or(0).min(GRAPH_MAX);
        self.labels.clear();
        for r in &self.branches {
            self.labels.entry(r.oid.clone()).or_default().push(r.clone());
        }
        self.author_w = AUTHOR_COL;
        // Every column has one width, whatever the commits are: the refs column too (when any commit has a ref).
        self.refs_need = self.commits.iter().filter_map(|k| self.labels.get(&k.oid)).map(|l| pills(l).iter().map(|(r, remotes)| ref_width(r, remotes)).sum::<usize>()).max().unwrap_or(0);
        self.refs_w = self.refs_need.min(REFS_COL);
        self.stashes = self.repo.stashes().unwrap_or_default();
        self.build_ref_rows(keep);
        for (i, len) in [self.ref_rows.len(), self.commits.len(), self.stashes.len()].into_iter().enumerate() {
            self.lists[i].cursor = self.lists[i].cursor.min(len.saturating_sub(1));
        }
        self.preview = None;
    }

    fn reload_all(&mut self) {
        self.files.refresh();
        self.reload_history();
    }

    /// Call after the editor the host opened for `Quit::Edit` returns.
    pub fn edited(&mut self, ok: bool) {
        self.reload_all();
        if !ok {
            self.say("the editor exited with an error", true);
        }
    }

    fn say(&mut self, msg: impl Into<String>, error: bool) {
        self.message = Some((msg.into(), error));
    }

    fn list_index(focus: Focus) -> Option<usize> {
        match focus {
            Focus::Files => None,
            Focus::Branches => Some(0),
            Focus::Commits => Some(1),
            Focus::Stashes => Some(2),
        }
    }

    fn len(&self, focus: Focus) -> usize {
        match focus {
            Focus::Files => 0,
            Focus::Branches => self.ref_rows.len(),
            Focus::Commits => self.commits.len(),
            Focus::Stashes => self.stashes.len(),
        }
    }

    /// Run a git change, report the outcome, reload everything.
    fn run(&mut self, ok: String, f: impl FnOnce(&Repo) -> Result<(), git_core::Error>) {
        match f(&self.repo) {
            Ok(()) => self.say(ok, false),
            Err(e) => self.say(e.to_string(), true),
        }
        self.reload_all();
    }

    // ---- the refs tree ----------------------------------------------------

    /// What identifies row `i` of the Refs tree across reloads: the key of a section or folder, the kind and name of a ref.
    fn ref_row_id(&self, i: usize) -> Option<String> {
        match self.ref_rows.get(i)? {
            RefRow::Section { key, .. } | RefRow::Folder { key, .. } => Some(key.clone()),
            RefRow::Item { index, .. } => self.branches.get(*index).map(|r| format!("{:?}:{}", r.kind, r.name)),
        }
    }

    /// The ref under the cursor of the Refs pane (`None` on a section or a folder).
    fn current_ref(&self) -> Option<&RefInfo> {
        match self.ref_rows.get(self.lists[0].cursor)? {
            RefRow::Item { index, .. } => self.branches.get(*index),
            _ => None,
        }
    }

    /// Lay the refs out as a tree: Branches, Remotes and Tags, each with its names split at `/` into folders
    /// (`feature/login` under `feature`, `origin/main` under `origin`). Folded sections and folders hide what is in them.
    fn build_ref_rows(&mut self, keep: Option<String>) {
        let mut rows = Vec::new();
        for (kind, label, tag) in [(RefKind::Branch, "Branches", "b"), (RefKind::Remote, "Remotes", "r"), (RefKind::Tag, "Tags", "t")] {
            let mut items: Vec<(usize, &RefInfo)> = self.branches.iter().enumerate().filter(|(_, r)| r.kind == kind).collect();
            if items.is_empty() {
                continue;
            }
            // Plain name order: the same folder stays together, `Feature/` and `feature/` are two folders.
            items.sort_by(|a, b| a.1.name.cmp(&b.1.name));
            let section = tag.to_string();
            rows.push(RefRow::Section { key: section.clone(), label, count: items.len() });
            if self.folded.contains(&section) {
                continue;
            }
            // The folders open above the previous ref, to emit only the ones that changed.
            let mut open: Vec<String> = Vec::new();
            let mut hidden_under: Option<usize> = None;
            for (index, r) in items {
                let parts: Vec<&str> = r.name.split('/').collect();
                let (folders, leaf) = parts.split_at(parts.len() - 1);
                let common = open.iter().zip(folders.iter()).take_while(|(a, b)| a.as_str() == **b).count();
                open.truncate(common);
                if hidden_under.is_some_and(|d| d > common) {
                    hidden_under = None;
                }
                for (d, name) in folders.iter().enumerate().skip(common) {
                    open.push((*name).to_string());
                    if hidden_under.is_some() {
                        continue;
                    }
                    let key = format!("{section}/{}", open.join("/"));
                    let count = self.branches.iter().filter(|b| b.kind == kind && b.name.starts_with(&format!("{}/", open.join("/")))).count();
                    rows.push(RefRow::Folder { key: key.clone(), name: (*name).to_string(), depth: d + 1, count });
                    if self.folded.contains(&key) {
                        hidden_under = Some(d + 1);
                    }
                }
                if hidden_under.is_none() {
                    rows.push(RefRow::Item { index, name: leaf[0].to_string(), depth: folders.len() + 1 });
                }
            }
        }
        // The cursor stays on the same row when it is still there.
        self.ref_rows = rows;
        let at = keep.and_then(|k| (0..self.ref_rows.len()).find(|&i| self.ref_row_id(i).as_deref() == Some(k.as_str()))).unwrap_or(self.lists[0].cursor);
        self.lists[0].cursor = at.min(self.ref_rows.len().saturating_sub(1));
    }

    /// Scroll the diff to the next or previous hunk (or file).
    /// Open the file tree of the commit under the cursor.
    fn open_commit_files(&mut self) {
        let Some(k) = self.commits.get(self.lists[1].cursor).cloned() else { return };
        match self.repo.changes(&k.oid) {
            Ok(changes) => {
                self.commit_files = Some(CommitFiles::new(k.oid, k.short, k.subject, changes));
                self.detail = true;
                self.preview = None;
                self.pscroll = 0;
            }
            Err(e) => self.say(e.to_string(), true),
        }
    }

    /// Keys while the files of a commit show: the tree moves and folds, esc goes back to the commits.
    fn commit_files_key(&mut self, key: &KeyEvent) -> Quit {
        use ratatui::crossterm::event::KeyModifiers;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let page = self.pheight.max(2);
        let Some(cf) = self.commit_files.as_mut() else { return Quit::No };
        let before = cf.cursor;
        match key.code {
            KeyCode::Char('c') if ctrl => return Quit::Yes,
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => self.commit_files = None,
            KeyCode::Char('j') | KeyCode::Down if !ctrl => cf.move_by(1),
            KeyCode::Char('k') | KeyCode::Up if !ctrl => cf.move_by(-1),
            KeyCode::Char('n') if ctrl => cf.move_by(1),
            KeyCode::Char('p') if ctrl => cf.move_by(-1),
            KeyCode::Char('j') if ctrl => cf.move_by(5),
            KeyCode::Char('k') if ctrl => cf.move_by(-5),
            KeyCode::PageDown => cf.move_by(page as isize),
            KeyCode::PageUp => cf.move_by(-(page as isize)),
            KeyCode::Char('g') | KeyCode::Home => cf.cursor = 0,
            KeyCode::Char('G') | KeyCode::End => cf.cursor = cf.rows.len().saturating_sub(1),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                if let Some(FileRow::Folder { path, folded: true, .. }) = cf.rows.get(cf.cursor) {
                    let p = path.clone();
                    cf.folded.remove(&p);
                    cf.rebuild();
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                // Fold an open folder; otherwise go to the parent folder; at the top, back to the commits.
                let depth_of = |r: &FileRow| match r {
                    FileRow::Folder { depth, .. } | FileRow::File { depth, .. } => *depth,
                };
                match cf.rows.get(cf.cursor) {
                    Some(FileRow::Folder { path, folded: false, .. }) => {
                        let p = path.clone();
                        cf.folded.insert(p);
                        cf.rebuild();
                    }
                    Some(row) if depth_of(row) > 0 => {
                        let want = depth_of(row) - 1;
                        if let Some(i) = cf.rows[..cf.cursor].iter().rposition(|r| matches!(r, FileRow::Folder { .. }) && depth_of(r) == want) {
                            cf.cursor = i;
                        }
                    }
                    _ => self.commit_files = None,
                }
            }
            KeyCode::Char('d') if ctrl => self.pscroll += (self.pheight / 2).max(1),
            KeyCode::Char('u') if ctrl => self.pscroll = self.pscroll.saturating_sub((self.pheight / 2).max(1)),
            KeyCode::Char('}') => self.jump_diff(true, false),
            KeyCode::Char('{') => self.jump_diff(false, false),
            KeyCode::Char(')') => self.jump_diff(true, true),
            KeyCode::Char('(') => self.jump_diff(false, true),
            _ => {}
        }
        if self.commit_files.as_ref().is_some_and(|cf| cf.cursor != before) {
            self.pscroll = 0;
        }
        Quit::No
    }

    /// Fold or unfold the file whose header is at (or above) the top of the diff.
    fn fold_file(&mut self) {
        let Some(p) = &self.preview else { return };
        let at = p.anchors.files.iter().rposition(|&m| m <= self.pscroll).or(if p.anchors.files.is_empty() { None } else { Some(0) });
        if let Some(path) = at.and_then(|i| p.anchors.paths.get(i)).cloned() {
            pv::diff_toggle_fold(&path);
            self.preview = None;
        }
    }

    fn jump_diff(&mut self, forward: bool, files: bool) {
        let Some(p) = &self.preview else { return };
        let marks = if files { &p.anchors.files } else { &p.anchors.hunks };
        let target = if forward { pv::Anchors::next(marks, self.pscroll) } else { pv::Anchors::prev(marks, self.pscroll) };
        if let Some(t) = target {
            self.pscroll = t;
        }
    }

    // ---- tabs and search --------------------------------------------------

    /// The tab that shows `focus`.
    fn screen_of(focus: Focus) -> Screen {
        match focus {
            Focus::Files => Screen::Status,
            Focus::Commits => Screen::Commits,
            Focus::Branches | Focus::Stashes => Screen::Refs,
        }
    }

    /// Switch tab (and focus its first pane).
    fn set_screen(&mut self, screen: Screen) {
        if screen == self.screen {
            return;
        }
        self.screen = screen;
        self.focus = match screen {
            Screen::Status => Focus::Files,
            Screen::Commits => Focus::Commits,
            Screen::Refs => Focus::Branches,
        };
        if screen != Screen::Commits {
            self.search = None;
        }
        self.pscroll = 0;
        // The commits are only loaded while their tab is shown.
        self.reload_history();
    }

    /// Show `focus`'s pane: its tab, and it focused.
    fn show(&mut self, focus: Focus) {
        self.set_screen(Self::screen_of(focus));
        self.set_focus(focus);
    }

    /// Reload the list after a filter changed and say how many commits are left.
    fn refilter(&mut self, what: &str) {
        self.lists[1] = ListState::default();
        self.pscroll = 0;
        self.reload_history();
        let n = self.commits.len();
        let shown = if self.filter.active() { format!("{what}: {n} commit{}", if n == 1 { "" } else { "s" }) } else { what.to_string() };
        self.say(shown, false);
    }

    /// Whether commit `i` of the list matches the search.
    fn commit_matches(&self, i: usize) -> bool {
        let Some(q) = self.search.as_deref().filter(|q| !q.is_empty()) else { return false };
        let q = q.to_lowercase();
        let Some(k) = self.commits.get(i) else { return false };
        let refs = self.labels.get(&k.oid).into_iter().flatten().map(|r| r.name.as_str()).collect::<Vec<_>>().join(" ");
        [k.oid.as_str(), k.subject.as_str(), k.author.as_str(), refs.as_str()].iter().any(|s| s.to_lowercase().contains(&q))
    }

    /// Go to the next (`step` 1) or previous (-1) commit matching the search, wrapping around.
    fn search_step(&mut self, step: isize) {
        let n = self.commits.len();
        if self.search.is_none() || n == 0 {
            self.say("no search: / starts one", true);
            return;
        }
        let from = self.lists[1].cursor;
        let found = (1..=n).map(|d| (from as isize + step * d as isize).rem_euclid(n as isize) as usize).find(|&i| self.commit_matches(i));
        let total = (0..n).filter(|&i| self.commit_matches(i)).count();
        match found {
            Some(i) => {
                self.lists[1].cursor = i;
                self.pscroll = 0;
                let nth = (0..=i).filter(|&j| self.commit_matches(j)).count();
                self.say(format!("match {nth} of {total}"), false);
            }
            None => self.say(format!("no commit matches \"{}\"", self.search.clone().unwrap_or_default()), true),
        }
    }

    // ---- events ---------------------------------------------------------

    pub fn handle_event(&mut self, ev: &Event) -> Quit {
        let Event::Key(key @ KeyEvent { kind, .. }) = ev else { return Quit::No };
        if *kind == KeyEventKind::Release {
            return Quit::No;
        }
        if let Some(help) = self.help.as_mut() {
            if !help.key(key) {
                self.help = None;
            }
            return Quit::No;
        }
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Prompt(kind, input) => {
                self.prompt_key(kind, input, ev, key);
                return Quit::No;
            }
            Mode::Confirm(c) => {
                self.confirm_key(c, key);
                return Quit::No;
            }
            Mode::Popup(p) => {
                self.popup_key(p, key);
                return Quit::No;
            }
            Mode::Normal => {}
        }
        if self.screen == Screen::Commits && self.commit_files.is_some() {
            return self.commit_files_key(key);
        }
        // The Files pane is the git panel: it has its own keys (and the global ones as commands).
        if self.focus == Focus::Files {
            return self.files_event(ev);
        }
        self.message = None;
        let li = Self::list_index(self.focus).expect("a list pane");
        match self.maps[li].feed(key) {
            Feed::Run(id) => self.run_id(id),
            Feed::NoBinding => {
                let seq = self.maps[li].rejected().to_string();
                self.say(format!("{seq} is not bound"), true);
                Quit::No
            }
            Feed::Pending | Feed::Cancelled | Feed::Unbound => Quit::No,
        }
    }

    /// Run an action by id: what a key, the leader menu or the palette resolves to.
    pub fn run_id(&mut self, id: &'static str) -> Quit {
        let focus = self.focus;
        let last = self.len(focus).saturating_sub(1);
        let li = Self::list_index(focus);
        let mut moved = true;
        match id {
            "tab.next" => self.set_screen(match self.screen {
                Screen::Status => Screen::Commits,
                Screen::Commits => Screen::Refs,
                Screen::Refs => Screen::Status,
            }),
            "tab.prev" => self.set_screen(match self.screen {
                Screen::Status => Screen::Refs,
                Screen::Commits => Screen::Status,
                Screen::Refs => Screen::Commits,
            }),
            "search.start" => self.mode = Mode::Prompt(PromptKind::Search, Input::new(self.search.clone().unwrap_or_default())),
            "search.next" => self.search_step(1),
            "search.prev" => self.search_step(-1),
            "view.detail" => self.detail = !self.detail,
            "filter.open" => self.open_filter(),
            "filter.clear" => {
                self.filter = Filter::default();
                self.refilter("no filters");
            }
            "focus.next" => self.focus_by(1),
            "focus.prev" => self.focus_by(-1),
            "focus.files" => self.show(Focus::Files),
            "focus.branches" => self.show(Focus::Branches),
            "focus.commits" => self.show(Focus::Commits),
            "focus.stashes" => self.show(Focus::Stashes),
            "back" => {
                // Esc: clear a search first, then go back to the Status tab.
                if self.work {
                    self.work = false;
                } else if self.compare.is_some() {
                    self.compare = None;
                } else if self.search.is_some() {
                    self.search = None;
                } else {
                    self.set_screen(Screen::Status);
                }
            }
            "quit" => return Quit::Yes,
            "help" => self.help = Some(ui::keys::Help::default()),
            "palette" => return Quit::Palette,
            "switch.to" => {
                let names = self.branches.iter().map(|b| b.name.clone()).collect();
                self.suggest = ui::prompt::Suggest::new(names);
                self.mode = Mode::Prompt(PromptKind::SwitchTo, Input::default());
            }
            "remote.fetch" => self.remote("fetching", "fetched", |r| r.fetch()),
            "remote.pull" => self.remote("pulling", "pulled", |r| r.pull()),
            "remote.push" => self.remote("pushing", "pushed", |r| r.push(false)),
            "remote.force_push" => self.mode = Mode::Confirm(Confirm::ForcePush),
            "op.continue" => self.run("continued".to_string(), |r| r.continue_operation()),
            "op.abort" => self.run("aborted".to_string(), |r| r.abort_operation()),
            "view.graph" => {
                self.graph = !self.graph;
                self.reload_history();
            }
            "view.hash" => self.cols.hash = !self.cols.hash,
            "view.author" => self.cols.author = !self.cols.author,
            "view.author_name" => self.cols.author_name = !self.cols.author_name,
            "view.refs" => self.cols.refs = !self.cols.refs,
            "view.subject" => self.cols.subject = !self.cols.subject,
            "view.rel_time" => self.cols.rel_time = !self.cols.rel_time,
            "view.abs_time" => self.cols.abs_time = !self.cols.abs_time,
            "view.all" => {
                self.all_branches = !self.all_branches;
                self.reload_history();
                self.say(if self.all_branches { "every branch" } else { "the checked-out branch" }, false);
            }
            "preview.down" => self.pscroll += (self.pheight / 2).max(1),
            "preview.up" => self.pscroll = self.pscroll.saturating_sub((self.pheight / 2).max(1)),
            "preview.next_hunk" => self.jump_diff(true, false),
            "preview.prev_hunk" => self.jump_diff(false, false),
            "preview.next_file" => self.jump_diff(true, true),
            "preview.prev_file" => self.jump_diff(false, true),
            "preview.fold" => self.fold_file(),
            "preview.whitespace" => {
                let on = !pv::diff_ignoring_whitespace();
                pv::diff_ignore_whitespace(on);
                self.preview = None;
                self.say(if on { "ignoring whitespace changes" } else { "showing whitespace changes" }, false);
            }
            "nav.up" | "nav.down" | "nav.up5" | "nav.down5" | "nav.page_up" | "nav.page_down" | "nav.top" | "nav.bottom" => {
                if let Some(li) = li {
                    let st = &mut self.lists[li];
                    st.cursor = match id {
                        "nav.up" => st.cursor.saturating_sub(1),
                        "nav.down" => (st.cursor + 1).min(last),
                        "nav.up5" => st.cursor.saturating_sub(5),
                        "nav.down5" => (st.cursor + 5).min(last),
                        "nav.page_up" => st.cursor.saturating_sub(10),
                        "nav.page_down" => (st.cursor + 10).min(last),
                        "nav.top" => 0,
                        _ => last,
                    };
                    self.pscroll = 0;
                }
            }
            _ => moved = false,
        }
        if !moved {
            self.pane_action(id);
        }
        Quit::No
    }

    /// Run a git command that talks to a remote on its own thread; the screen shows `label …`
    /// and `tick` finishes it. One at a time.
    fn remote(&mut self, label: &str, done: &str, f: impl FnOnce(&Repo) -> Result<(), git_core::Error> + Send + 'static) {
        if self.job.is_some() {
            self.say("busy: wait for the current remote command", true);
            return;
        }
        let root = self.root.clone();
        let handle = std::thread::spawn(move || match Repo::discover(&root) {
            Some(repo) => f(&repo),
            None => Err(git_core::Error("the repository is gone".into())),
        });
        self.job = Some(Job { label: label.to_string(), done: done.to_string(), handle });
    }

    /// Call between events: finishes a remote command that ended. `true` when the screen should be redrawn.
    pub fn tick(&mut self) -> bool {
        if self.watch.as_ref().is_some_and(|w| w.take()) {
            self.reload_all();
            return true;
        }
        if self.job.as_ref().is_some_and(|j| j.handle.is_finished()) {
            let job = self.job.take().expect("checked above");
            match job.handle.join() {
                Ok(Ok(())) => self.say(job.done, false),
                Ok(Err(e)) => self.say(e.to_string(), true),
                Err(_) => self.say("the remote command crashed", true),
            }
            self.reload_all();
            return true;
        }
        self.job.is_some()
    }

    /// The operation that stopped and waits (conflicts), shown in the status line.
    fn operation_name(&self) -> Option<&'static str> {
        match self.repo.operation()? {
            Operation::Merge => Some("MERGING"),
            Operation::Rebase => Some("REBASING"),
            Operation::CherryPick => Some("CHERRY-PICKING"),
            Operation::Revert => Some("REVERTING"),
        }
    }

    fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
        self.pscroll = 0;
    }

    /// Move the focus to the next or previous pane of the tab (only the Refs tab has two).
    fn focus_by(&mut self, delta: isize) {
        let panes: &[Focus] = match self.screen {
            Screen::Status => &STATUS_PANES,
            Screen::Commits => &COMMITS_PANES,
            Screen::Refs => &REFS_PANES,
        };
        let at = panes.iter().position(|f| *f == self.focus).unwrap_or(0) as isize;
        self.focus = panes[(at + delta).rem_euclid(panes.len() as isize) as usize];
        self.pscroll = 0;
    }

    fn files_event(&mut self, ev: &Event) -> Quit {
        let action = self.files.handle_event(ev);
        self.files_action(action)
    }

    fn files_action(&mut self, action: PanelAction) -> Quit {
        match action {
            PanelAction::Close => Quit::Yes,
            PanelAction::Open(path) => Quit::Edit(path),
            PanelAction::Changed => {
                self.reload_history();
                Quit::No
            }
            PanelAction::Palette => Quit::Palette,
            PanelAction::Help => {
                self.help = Some(ui::keys::Help::default());
                Quit::No
            }
            PanelAction::Command(id) => self.run_id(id),
            PanelAction::None => Quit::No,
        }
    }

    /// Every action of the focused pane with its keys, for the palette and the help.
    pub fn keys_rows(&self) -> Vec<ui::keys::Row> {
        match Self::list_index(self.focus) {
            Some(li) => self.maps[li].rows(),
            None => self.files.keys_rows(),
        }
    }

    /// Run the action the palette picked.
    pub fn run_palette(&mut self, id: &'static str) -> Quit {
        match self.focus {
            Focus::Files => {
                let action = self.files.run(id);
                self.files_action(action)
            }
            _ => self.run_id(id),
        }
    }

    /// The actions that act on the highlighted branch or stash.
    fn pane_action(&mut self, id: &str) {
        match self.focus {
            Focus::Branches => {
                // Renaming a folder renames everything under it (a remote folder renames the remote itself).
                if id == "branch.rename" {
                    if let Some(RefRow::Folder { key, depth, .. }) = self.ref_rows.get(self.lists[0].cursor).cloned() {
                        let (section, path) = key.split_once('/').unwrap_or(("", &key));
                        match section {
                            "b" => self.mode = Mode::Prompt(PromptKind::RenameFolder { kind: RefKind::Branch, prefix: path.to_string() }, Input::new(path.to_string())),
                            "t" => self.mode = Mode::Prompt(PromptKind::RenameFolder { kind: RefKind::Tag, prefix: path.to_string() }, Input::new(path.to_string())),
                            _ if depth == 1 => self.mode = Mode::Prompt(PromptKind::RenameRemote(path.to_string()), Input::new(path.to_string())),
                            _ => self.say("rename a remote branch itself, a remote folder is only part of its name", true),
                        }
                        return;
                    }
                    if matches!(self.ref_rows.get(self.lists[0].cursor), Some(RefRow::Section { .. })) {
                        self.say("pick a ref or a folder to rename", true);
                        return;
                    }
                }
                // On a section or folder the Enter key folds or unfolds it.
                if let Some(RefRow::Section { key, .. } | RefRow::Folder { key, .. }) = self.ref_rows.get(self.lists[0].cursor) {
                    if id == "branch.checkout" {
                        let key = key.clone();
                        if !self.folded.remove(&key) {
                            self.folded.insert(key);
                        }
                        let keep = self.ref_row_id(self.lists[0].cursor);
                        self.build_ref_rows(keep);
                        return;
                    }
                }
                let Some(b) = self.current_ref().cloned() else {
                    if id == "branch.new" {
                        self.mode = Mode::Prompt(PromptKind::NewBranch, Input::default());
                    }
                    return;
                };
                match id {
                    "branch.checkout" => {
                        if b.is_head {
                            self.say(format!("already on {}", b.name), false);
                        } else {
                            let (name, kind) = (b.name.clone(), b.kind);
                            self.run(format!("switched to {}", b.name), move |r| r.checkout_ref(&name, kind));
                        }
                    }
                    "branch.new" => self.mode = Mode::Prompt(PromptKind::NewBranch, Input::default()),
                    "branch.delete" | "branch.force_delete" => match b.kind {
                        RefKind::Tag => self.mode = Mode::Confirm(Confirm::DeleteTag(b.name)),
                        RefKind::Remote => self.mode = Mode::Confirm(Confirm::DeleteRemote(b.name)),
                        RefKind::Branch if b.is_head => self.say("the checked-out branch cannot be deleted", true),
                        RefKind::Branch => self.mode = Mode::Confirm(Confirm::DeleteBranch { name: b.name, force: id == "branch.force_delete" }),
                    },
                    "branch.rebase" => {
                        if b.is_head {
                            self.say("cannot rebase a branch onto itself", true);
                        } else {
                            self.mode = Mode::Confirm(Confirm::Rebase(b.name));
                        }
                    }
                    "branch.rename" => {
                        let kind = match b.kind {
                            RefKind::Branch => PromptKind::RenameBranch(b.name.clone()),
                            RefKind::Tag => PromptKind::RenameTag(b.name.clone()),
                            RefKind::Remote => PromptKind::RenameRemoteBranch(b.name.clone()),
                        };
                        // A remote branch is renamed without its remote (`topic`, not `origin/topic`).
                        let shown = if b.kind == RefKind::Remote { b.name.split_once('/').map(|(_, n)| n.to_string()).unwrap_or_default() } else { b.name };
                        self.mode = Mode::Prompt(kind, Input::new(shown));
                    }
                    "branch.merge" => {
                        if b.is_head {
                            self.say("cannot merge a branch into itself", true);
                        } else {
                            self.mode = Mode::Confirm(Confirm::Merge(b.name));
                        }
                    }
                    _ => {}
                }
            }
            Focus::Commits => {
                let Some(k) = self.commits.get(self.lists[1].cursor).cloned() else { return };
                match id {
                    "commit.copy_hash" => match ui::clipboard::copy(&k.oid) {
                        Ok(()) => self.say(format!("copied {} to the clipboard", k.short), false),
                        Err(e) => self.say(format!("could not copy: {e}"), true),
                    },
                    "commit.files" => self.open_commit_files(),
                    "commit.compare_work" => {
                        self.work = !self.work;
                        self.compare = None;
                        self.detail = true;
                        self.say(if self.work { "diff from the commit to the work tree (M or Esc clears)" } else { "compare cleared" }, false);
                    }
                    "commit.compare" => {
                        self.work = false;
                        if self.compare.as_deref() == Some(k.oid.as_str()) {
                            self.compare = None;
                            self.say("compare cleared", false);
                        } else {
                            self.compare = Some(k.oid.clone());
                            self.detail = true;
                            self.say(format!("comparing with {}: move to another commit (m clears)", k.short), false);
                        }
                    }
                    "commit.checkout" => {
                        let oid = k.oid.clone();
                        self.run(format!("checked out {}", k.short), move |r| r.checkout_commit(&oid));
                    }
                    "commit.cherry_pick" => {
                        let oid = k.oid.clone();
                        self.run(format!("cherry-picked {}", k.short), move |r| r.cherry_pick(&oid));
                    }
                    "commit.revert" => self.mode = Mode::Confirm(Confirm::Revert(k.oid)),
                    "commit.reword" => {
                        self.mode = Mode::Prompt(PromptKind::Reword(k.oid), Input::new(k.subject));
                    }
                    "commit.amend" => {
                        let head = self.commits.first().map(|c| c.subject.clone()).unwrap_or_default();
                        self.mode = Mode::Prompt(PromptKind::Amend(head.clone()), Input::new(head));
                    }
                    "commit.branch" => self.mode = Mode::Prompt(PromptKind::BranchAt(k.oid), Input::default()),
                    "commit.tag" => self.mode = Mode::Prompt(PromptKind::TagAt(k.oid), Input::default()),
                    "commit.reset_soft" => self.run(format!("reset (soft) to {}", k.short), move |r| r.reset(&k.oid, ResetMode::Soft)),
                    "commit.reset_mixed" => self.run(format!("reset (mixed) to {}", k.short), move |r| r.reset(&k.oid, ResetMode::Mixed)),
                    "commit.reset_hard" => self.mode = Mode::Confirm(Confirm::Reset { oid: k.oid, mode: ResetMode::Hard }),
                    _ => {}
                }
            }
            Focus::Stashes => {
                let cur = self.stashes.get(self.lists[2].cursor).map(|s| s.index);
                match (id, cur) {
                    ("stash.new", _) => self.mode = Mode::Prompt(PromptKind::NewStash, Input::default()),
                    ("stash.apply", Some(i)) => self.run(format!("applied stash@{{{i}}}"), move |r| r.stash_apply(i)),
                    ("stash.pop", Some(i)) => self.run(format!("popped stash@{{{i}}}"), move |r| r.stash_pop(i)),
                    ("stash.drop", Some(i)) => self.mode = Mode::Confirm(Confirm::DropStash(i)),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// The filter popup: message, authors, categories, branches and date, with what is set chosen.
    fn open_filter(&mut self) {
        use ui::multi::{Kind, Multi, Section};
        // The authors and categories of the history, the most frequent first.
        let (mut authors, mut cats): (Vec<(usize, String)>, Vec<(usize, String)>) = (Vec::new(), Vec::new());
        let bump = |list: &mut Vec<(usize, String)>, name: String| match list.iter_mut().find(|(_, n)| *n == name) {
            Some((n, _)) => *n += 1,
            None => list.push((1, name)),
        };
        for k in self.repo.commits_all(TAB_LOG_LIMIT).unwrap_or_default() {
            bump(&mut authors, k.author.clone());
            if let Some(w) = ui::subject::category_word(&k.subject) {
                bump(&mut cats, w);
            }
        }
        let ranked = |mut v: Vec<(usize, String)>| -> Vec<String> {
            v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            v.into_iter().map(|(_, n)| n).collect()
        };
        let f = &self.filter;
        let sections = vec![
            Section::new("Message", Kind::Text, Vec::new(), if f.text.is_empty() { Vec::new() } else { vec![f.text.clone()] }),
            Section::new("Author", Kind::Many, ranked(authors), f.authors.clone()),
            Section::new("Category", Kind::Many, ranked(cats), f.categories.clone()),
            Section::new("Branch", Kind::Many, self.branches.iter().map(|b| b.name.clone()).collect(), f.branches.clone()),
            Section::new("Since", Kind::One, ["1d", "3d", "7d", "2w", "1m", "3m", "6m", "1y"].map(String::from).to_vec(), f.since.iter().map(|(s, _)| s.clone()).collect()),
        ];
        self.mode = Mode::Popup(Multi::new("Filter commits", sections));
    }

    fn popup_key(&mut self, mut popup: ui::multi::Multi, key: &KeyEvent) {
        match popup.key(key) {
            ui::multi::Outcome::Open => self.mode = Mode::Popup(popup),
            ui::multi::Outcome::Cancel => {}
            ui::multi::Outcome::Apply => {
                self.filter.text = popup.chosen("Message").first().cloned().unwrap_or_default();
                self.filter.authors = popup.chosen("Author").to_vec();
                self.filter.categories = popup.chosen("Category").to_vec();
                self.filter.branches = popup.chosen("Branch").to_vec();
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
                self.filter.since = popup.chosen("Since").first().and_then(|s| parse_since(s, now).map(|t| (s.clone(), t)));
                self.refilter("filtered");
            }
        }
    }

    fn prompt_key(&mut self, kind: PromptKind, mut input: Input, ev: &Event, key: &KeyEvent) {
        // Filter prompts offer options: up and down choose, tab fills the line in.
        if matches!(kind, PromptKind::SwitchTo) {
            let ctrl = key.modifiers.contains(ratatui::crossterm::event::KeyModifiers::CONTROL);
            let step = match key.code {
                KeyCode::Down | KeyCode::Tab => Some(1),
                KeyCode::Up | KeyCode::BackTab => Some(-1),
                KeyCode::Char('n') if ctrl => Some(1),
                KeyCode::Char('p') if ctrl => Some(-1),
                _ => None,
            };
            if let Some(d) = step {
                self.suggest.typed(input.value());
                if matches!(key.code, KeyCode::Tab) {
                    if let Some(s) = self.suggest.current() {
                        input = Input::new(s);
                    }
                } else {
                    self.suggest.step(d);
                }
                self.mode = Mode::Prompt(kind, input);
                return;
            }
        }
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Enter => {
                let value = input.value().to_string();
                match kind {
                    PromptKind::Search => {
                        self.search = if value.is_empty() { None } else { Some(value) };
                        if self.search.is_some() {
                            // The commit under the cursor counts if it matches.
                            let at = self.lists[1].cursor;
                            if self.commit_matches(at) {
                                let total = (0..self.commits.len()).filter(|&i| self.commit_matches(i)).count();
                                let nth = (0..=at).filter(|&j| self.commit_matches(j)).count();
                                self.say(format!("match {nth} of {total}"), false);
                            } else {
                                self.search_step(1);
                            }
                        }
                    }
                    PromptKind::NewBranch => {
                        let v = value.clone();
                        self.run(format!("created {value}"), move |r| r.create_branch(&v));
                    }
                    PromptKind::NewStash => {
                        let v = value.clone();
                        self.run("stashed the changes".to_string(), move |r| r.stash_push(&v));
                    }
                    PromptKind::RenameBranch(old) => {
                        let v = value.clone();
                        self.run(format!("renamed {old} to {value}"), move |r| r.rename_branch(&old, &v));
                    }
                    PromptKind::RenameTag(old) => {
                        let v = value.clone();
                        self.run(format!("renamed {old} to {value}"), move |r| r.rename_tag(&old, &v));
                    }
                    PromptKind::RenameRemote(old) => {
                        let v = value.clone();
                        self.run(format!("renamed remote {old} to {value}"), move |r| r.rename_remote(&old, &v));
                    }
                    PromptKind::RenameRemoteBranch(old) => {
                        let v = value.clone();
                        let done = format!("renamed {old} to {value} on the remote");
                        self.remote("renaming on the remote", &done, move |r| r.rename_remote_branch(&old, &v));
                    }
                    PromptKind::RenameFolder { kind, prefix } => {
                        let value = value.trim_end_matches('/').to_string();
                        let pairs: Vec<(String, String)> = self.branches.iter().filter(|r| r.kind == kind && r.name.starts_with(&format!("{prefix}/"))).map(|r| (r.name.clone(), format!("{value}{}", &r.name[prefix.len()..]))).collect();
                        let n = pairs.len();
                        // The names are checked and the renames are all or none (`Repo::rename_many`).
                        if value.is_empty() || value.starts_with('/') || value.contains("//") {
                            self.say("the folder name is empty or has an empty part", true);
                        } else {
                            self.run(format!("renamed {n} ref{} to {value}/", if n == 1 { "" } else { "s" }), move |r| r.rename_many(kind, &pairs));
                        }
                    }
                    PromptKind::BranchAt(oid) => {
                        let v = value.clone();
                        self.run(format!("created {value}"), move |r| r.create_branch_at(&v, &oid));
                    }
                    PromptKind::TagAt(oid) => {
                        let v = value.clone();
                        self.run(format!("tagged {value}"), move |r| r.create_tag(&v, &oid));
                    }
                    PromptKind::Reword(oid) => {
                        let v = value.clone();
                        self.run("reworded".to_string(), move |r| r.reword_commit(&oid, &v));
                    }
                    PromptKind::SwitchTo => self.switch_to(value),
                    PromptKind::Amend(old) => {
                        let v = value.clone();
                        self.run("amended the last commit".to_string(), move |r| if v == old { r.amend(None) } else { r.amend(Some(&v)) });
                    }
                }
            }
            _ => {
                input.handle_event(ev);
                self.suggest.typed(input.value());
                self.mode = Mode::Prompt(kind, input);
            }
        }
    }

    /// Check out what was typed: a ref by its name, the option chosen from the list, or a commit.
    fn switch_to(&mut self, typed: String) {
        let typed = typed.trim().to_string();
        let by = |name: &str| self.branches.iter().find(|b| b.name == name).map(|b| (b.name.clone(), b.kind, b.is_head));
        let pick = by(&typed).or_else(|| self.suggest.current().and_then(|s| by(&s)));
        match pick {
            Some((name, _, true)) => self.say(format!("already on {name}"), false),
            Some((name, kind, false)) => self.run(format!("switched to {name}"), move |r| r.checkout_ref(&name, kind)),
            None if typed.is_empty() => {}
            None => match self.repo.commit(&typed) {
                Ok(k) => {
                    let (oid, short) = (k.oid.clone(), k.short.clone());
                    self.run(format!("checked out {short}"), move |r| r.checkout_commit(&oid));
                }
                Err(_) => self.say(format!("no branch, tag or commit named {typed}"), true),
            },
        }
    }

    fn confirm_key(&mut self, c: Confirm, key: &KeyEvent) {
        let again = matches!(c, Confirm::DeleteBranch { .. } | Confirm::DeleteTag(_) | Confirm::DeleteRemote(_) | Confirm::DropStash(_));
        if !(matches!(key.code, KeyCode::Char('y' | 'Y') | KeyCode::Enter) || (again && matches!(key.code, KeyCode::Char('d' | 'D')))) {
            return;
        }
        match c {
            Confirm::DeleteBranch { name, force } => {
                let n = name.clone();
                self.run(format!("deleted {name}"), move |r| r.delete_branch(&n, force));
            }
            Confirm::Merge(name) => {
                let n = name.clone();
                self.run(format!("merged {name}"), move |r| r.merge(&n));
            }
            Confirm::DropStash(i) => self.run(format!("dropped stash@{{{i}}}"), move |r| r.stash_drop(i)),
            Confirm::Rebase(name) => {
                let n = name.clone();
                self.run(format!("rebased onto {name}"), move |r| r.rebase(&n));
            }
            Confirm::Revert(oid) => self.run("reverted".to_string(), move |r| r.revert(&oid)),
            Confirm::Reset { oid, mode } => self.run("reset (hard)".to_string(), move |r| r.reset(&oid, mode)),
            Confirm::ForcePush => self.remote("force-pushing", "force-pushed", |r| r.push(true)),
            Confirm::DeleteTag(name) => {
                let n = name.clone();
                self.run(format!("deleted tag {name}"), move |r| r.delete_tag(&n));
            }
            Confirm::DeleteRemote(name) => {
                let n = name.clone();
                self.remote("deleting", "deleted", move |r| r.delete_remote_branch(&n));
            }
        }
    }

    // ---- drawing --------------------------------------------------------

    /// Draw the tab strip and the screen of the active tab.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let labels = [ui::tabs::Label::new("Status"), ui::tabs::Label::new("Commits"), ui::tabs::Label::new("Refs")];
        let active = match self.screen {
            Screen::Status => 0,
            Screen::Commits => 1,
            Screen::Refs => 2,
        };
        ui::tabs::render(Rect { height: 1, ..area }, buf, &theme, &labels, active, true);
        let rest = Rect { y: area.y + 1, height: area.height.saturating_sub(1), ..area };
        match self.screen {
            Screen::Status => self.render_panes(rest, buf),
            Screen::Commits => self.render_commits_tab(rest, buf),
            Screen::Refs => self.render_refs_tab(rest, buf),
        }
    }

    /// The Commits tab: the whole graph of every branch full width, the diff of the commit beside it on request.
    fn render_commits_tab(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        if self.commit_files.is_some() {
            let [left, right] = ui::layout::main_preview(&theme, body);
            self.render_commit_files(left, buf, &theme);
            self.render_preview(Focus::Commits, right, buf, &theme);
        } else if self.detail && body.width >= 60 {
            let [left, right] = ui::layout::main_preview(&theme, body);
            self.render_list(Focus::Commits, left, buf, &theme);
            self.render_preview(Focus::Commits, right, buf, &theme);
        } else {
            self.render_list(Focus::Commits, body, buf, &theme);
        }
        self.render_status(status, buf, &theme);
        self.render_overlays(area, buf, &theme);
    }

    /// The files of the commit being explored, as a tree.
    fn render_commit_files(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let Some(cf) = self.commit_files.as_mut() else { return };
        let c = &theme.colors;
        let title = format!("{}  {}", cf.short, cf.subject);
        let pane = Pane::new(theme).title(&title).icon(ui::icons::COMMIT).focused(true);
        let inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height == 0 || inner.width < 6 {
            return;
        }
        if cf.rows.is_empty() {
            buf.set_string(inner.x + 1, inner.y, "no files changed", Style::new().fg(c.muted.into()));
            return;
        }
        let page = usize::from(inner.height);
        if cf.cursor < cf.offset {
            cf.offset = cf.cursor;
        } else if cf.cursor >= cf.offset + page {
            cf.offset = cf.cursor + 1 - page;
        }
        let geo = ui::list::Rows::new(area, inner);
        let git_color = |k: git_core::Kind| -> ratatui::style::Color {
            let g = &c.git;
            match k {
                git_core::Kind::Added => g.added,
                git_core::Kind::Modified | git_core::Kind::Typechange => g.modified,
                git_core::Kind::Deleted => g.deleted,
                git_core::Kind::Renamed => g.renamed,
                git_core::Kind::Untracked => g.untracked,
                git_core::Kind::Conflict => g.conflict,
            }
            .into()
        };
        for (n, row) in cf.rows.iter().enumerate().skip(cf.offset).take(page) {
            let y = inner.y + (n - cf.offset) as u16;
            let selected = n == cf.cursor;
            let file_row = match row {
                FileRow::Folder { label, depth, count, folded, .. } => ui::tree::FileRow { depth: *depth, name: label, is_dir: true, open: !*folded, count: Some(*count), ..Default::default() },
                FileRow::File { change, depth } => {
                    let color = git_color(change.kind);
                    // The line counts, left of the sign (when there is room for them).
                    let trailing = if inner.width < 30 {
                        Vec::new()
                    } else if change.binary {
                        vec![Span::styled("binary ", Style::new().fg(c.muted.into()))]
                    } else {
                        vec![Span::styled(format!("+{}", change.added), Style::new().fg(c.git.added.into())), Span::styled(format!(" -{} ", change.removed), Style::new().fg(c.git.deleted.into()))]
                    };
                    ui::tree::FileRow {
                        depth: *depth,
                        name: change.path.rsplit('/').next().unwrap_or(&change.path),
                        tint: Some(color),
                        trailing,
                        signs: vec![(change.kind.letter(), color)],
                        ..Default::default()
                    }
                }
            };
            ui::tree::file_row(theme, buf, &geo, y, &ui::tree::FileRow { selected, focused: true, ..file_row });
        }
    }

    /// The Status tab: the changed files and the preview of the one under the cursor.
    fn render_panes(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        let [left, right] = ui::layout::main_preview(&theme, body);
        self.files.render_compact(left, buf, &theme, true);
        self.files.render_preview_pane(right, buf, &theme);
        self.render_status(status, buf, &theme);
        self.render_overlays(area, buf, &theme);
    }

    /// The Refs tab: branches (with remotes and tags) above the stashes, the preview beside them.
    fn render_refs_tab(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        let [left, right] = ui::layout::main_preview(&theme, body);
        // The focused pane takes the room, the other shows a few rows.
        let constraints = REFS_PANES.map(|f| if f == self.focus { Constraint::Min(7) } else { Constraint::Length(6) });
        let panes = Layout::vertical(constraints).split(left);
        for (pane, f) in panes.iter().zip(REFS_PANES) {
            self.render_list(f, *pane, buf, &theme);
            if f != self.focus {
                ui::dim::dim_to(buf, *pane, &theme, ui::dim::KEEP_INACTIVE);
            }
        }
        self.render_preview(self.focus, right, buf, &theme);
        self.render_status(status, buf, &theme);
        self.render_overlays(area, buf, &theme);
    }

    /// The which-key menu and the keys help, over everything else.
    fn render_overlays(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let menu = match self.focus {
            Focus::Files => self.files.menu(),
            f => Self::list_index(f).and_then(|i| self.maps[i].menu()),
        };
        if let Some(menu) = menu {
            ui::keys::render_menu(area, buf, theme, &menu);
        }
        if let Mode::Popup(p) = &self.mode {
            p.render(area, buf, theme);
        }
        if self.help.is_some() {
            let rows = self.keys_rows();
            if let Some(h) = self.help.as_mut() {
                h.render(area, buf, theme, "Keys", &rows);
            }
        }
    }

    fn render_list(&mut self, which: Focus, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let (title, n) = match which {
            Focus::Branches => ("Refs", self.ref_rows.len()),
            Focus::Commits => ("Commits", self.commits.len()),
            _ => ("Stashes", self.stashes.len()),
        };
        let focused = self.focus == which;
        let mut pane = Pane::new(theme).title(title).focused(focused);
        let icon = if which == Focus::Branches { Some(ui::icons::BRANCH) } else { ui::icons::for_title(title) };
        if let Some(icon) = icon {
            pane = pane.icon(icon);
        }
        let mut inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height == 0 || inner.width < 4 {
            return;
        }
        // The filters in force, on a row of their own above the list.
        if which == Focus::Commits && self.filter.active() {
            let c = &theme.colors;
            let mut spans = vec![Span::styled("filter ", Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD))];
            for (i, (what, value)) in self.filter.parts().into_iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled("  ·  ", Style::new().fg(c.muted.into())));
                }
                spans.push(Span::styled(format!("{what} "), Style::new().fg(c.muted.into())));
                spans.push(Span::styled(value, Style::new().fg(c.warning.into())));
            }
            Paragraph::new(Line::from(spans)).render(Rect { height: 1, ..inner }, buf);
            inner.y += 1;
            inner.height -= 1;
            if inner.height == 0 {
                return;
            }
        }
        let Some(li) = Self::list_index(which) else { return };
        if which == Focus::Commits {
            self.fit_columns(inner.width);
        }
        if n == 0 {
            buf.set_string(inner.x + 1, inner.y, "nothing here", Style::new().fg(theme.colors.muted.into()));
            return;
        }
        let page = usize::from(inner.height);
        let st = &mut self.lists[li];
        if st.cursor < st.offset {
            st.offset = st.cursor;
        } else if st.cursor >= st.offset + page {
            st.offset = st.cursor + 1 - page;
        }
        let (cursor, offset) = (st.cursor, st.offset);
        let rows = ui::list::Rows::new(area, inner);
        for (row, i) in (offset..(offset + page).min(n)).enumerate() {
            let y = inner.y + row as u16;
            let base = ui::list::style(theme, i == cursor && focused, None);
            buf.set_style(rows.bar(y), base);
            let (left, right) = self.row(which, i, theme);
            let content = rows.content(y);
            let rw = right.chars().count() as u16;
            let room = if rw > 0 && content.width > rw + 12 { content.width - rw - 1 } else { content.width };
            Paragraph::new(left).style(base).render(Rect { width: room, ..content }, buf);
            if room < content.width {
                buf.set_string(content.right() - rw, y, right, base.fg(theme.colors.muted.into()));
            }
        }
    }

    /// Give the flexible columns their widths for a list `width` wide: the refs column as wide as the most
    /// refs on one commit need (as far as the message keeps room), and when the message is hidden the
    /// refs (or the author names) take what is left, so the row is always filled.
    fn fit_columns(&mut self, width: u16) {
        const MIN_SUBJECT: usize = 24;
        let width = usize::from(width);
        let mut fixed = if self.graph { self.graph_w + 1 } else { 0 };
        if self.cols.hash {
            fixed += HASH_W + 1;
        }
        if self.cols.rel_time {
            fixed += 9;
        }
        if self.cols.abs_time {
            fixed += 18;
        }
        let name_w = if self.cols.author_name { AUTHOR_COL } else { 2 };
        if self.cols.author {
            fixed += name_w + 1;
        }
        self.author_w = name_w;
        let free = width.saturating_sub(fixed);
        let refs_on = self.cols.refs && self.refs_need > 0;
        self.refs_w = match (refs_on, self.cols.subject) {
            (false, _) => 0,
            (true, true) => self.refs_need.min(free.saturating_sub(MIN_SUBJECT)).max(8.min(self.refs_need)),
            // No message: the refs take the rest of the row.
            (true, false) => free.saturating_sub(1).max(self.refs_need.min(free)),
        };
        if !self.cols.subject && !refs_on && self.cols.author && self.cols.author_name {
            // Only names are left to fill the row.
            self.author_w = free.max(AUTHOR_COL);
        }
    }

    /// The text of row `i`: the entry, and its age for the right edge.
    fn row(&self, which: Focus, i: usize, theme: &Theme) -> (Line<'static>, String) {
        let c = &theme.colors;
        let fg = Style::new().fg(c.fg.into());
        let muted = Style::new().fg(c.muted.into());
        match which {
            Focus::Branches => {
                let accent = Style::new().fg(c.accent.into());
                match &self.ref_rows[i] {
                    RefRow::Section { key, label, count } => {
                        let mut spans = ui::tree::group(theme, 0, self.folded.contains(key), None, label, accent.add_modifier(Modifier::BOLD));
                        spans.push(ui::tree::count(theme, *count));
                        (Line::from(spans), String::new())
                    }
                    RefRow::Folder { key, name, depth, count } => {
                        let mut spans = ui::tree::dir(theme, *depth, name, !self.folded.contains(key));
                        spans.push(ui::tree::count(theme, *count));
                        (Line::from(spans), String::new())
                    }
                    RefRow::Item { index, name, depth } => {
                        let b = &self.branches[*index];
                        let style = if b.is_head {
                            Style::new().fg(c.success.into()).add_modifier(Modifier::BOLD)
                        } else if b.kind == RefKind::Tag {
                            Style::new().fg(c.warning.into())
                        } else {
                            fg
                        };
                        // Indentation guides, like the files tree: a thin line at each level.
                        (Line::from(vec![ui::tree::guides(theme, *depth), Span::styled(name.clone(), style)]), ago(b.time))
                    }
                }
            }
            Focus::Commits => {
                let k = &self.commits[i];
                // `hash AB subject`: the author as two initials in a color of their own, like lazygit.
                let who = if self.cols.author_name { let mut n: String = k.author.chars().take(self.author_w).collect(); n.push_str(&" ".repeat(self.author_w.saturating_sub(n.chars().count()))); n } else { format!("{:<2}", initials(&k.author)) };
                let author = Span::styled(format!("{who} "), Style::new().fg(author_color(theme, &k.author)));
                let hit = self.commit_matches(i);
                let mut spans = Vec::new();
                if let Some(row) = self.graph_rows.get(i) {
                    spans.extend(self.graph_spans(row, theme));
                }
                // A table: graph, author, hash, refs (pills; the checked-out branch is the solid one
                // with the branch icon), subject; every column starts at the same place in every row.
                if self.cols.author {
                    spans.push(author);
                }
                if self.cols.hash {
                    spans.push(Span::styled(format!("{:<w$} ", k.short, w = HASH_W), if self.compare.as_deref() == Some(k.oid.as_str()) { Style::new().fg(theme.colors.accent.into()).add_modifier(Modifier::BOLD | Modifier::UNDERLINED) } else { muted }));
                }
                if self.cols.refs && self.refs_w > 0 {
                    let mut used = 0;
                    let refs = pills(self.labels.get(&k.oid).map(Vec::as_slice).unwrap_or_default());
                    for (n, (r, remotes)) in refs.iter().enumerate() {
                        let w = ref_width(r, remotes);
                        if used + w > self.refs_w {
                            // The rest does not fit: say how many are left.
                            let more = format!("+{}", refs.len() - n);
                            if used + more.len() <= self.refs_w {
                                used += more.len();
                                spans.push(Span::styled(more, muted));
                            }
                            break;
                        }
                        spans.extend(ref_pill(theme, r, remotes));
                        used += w;
                    }
                    spans.push(Span::raw(" ".repeat(self.refs_w.saturating_sub(used) + 1)));
                }
                if !self.cols.subject {
                } else if hit {
                    spans.push(Span::styled(k.subject.clone(), Style::new().fg(c.warning.into()).add_modifier(Modifier::BOLD)));
                } else {
                    spans.extend(ui::subject::spans(theme, &k.subject, fg));
                }
                let mut when = Vec::new();
                if self.cols.abs_time {
                    when.push(format!("{:<16}", pv::format_utc(k.time)));
                }
                if self.cols.rel_time {
                    when.push(format!("{:>8}", ago(k.time)));
                }
                (Line::from(spans), when.join("  "))
            }
            _ => {
                let s = &self.stashes[i];
                (Line::from(vec![Span::styled(format!("{} ", s.index), muted), Span::styled(s.message.clone(), fg)]), ago(s.time))
            }
        }
    }

    /// The graph cells of a row as coloured spans, padded to the width of the column.
    fn graph_spans(&self, row: &git_core::GraphRow, theme: &Theme) -> Vec<Span<'static>> {
        let c = &theme.colors;
        let palette = [c.accent, c.success, c.warning, c.info, c.error, c.git.renamed];
        let mut spans: Vec<Span<'static>> = row.cells.iter().take(self.graph_w).map(|(ch, lane)| Span::styled(ch.to_string(), Style::new().fg(palette[lane % palette.len()].into()))).collect();
        let used = spans.len();
        spans.push(Span::raw(" ".repeat(self.graph_w.saturating_sub(used) + 1)));
        spans
    }

    /// The rev to preview for the highlighted entry of a list pane.
    fn current_rev(&self, which: Focus) -> Option<String> {
        match which {
            Focus::Branches => self.current_ref().map(|b| b.name.clone()),
            Focus::Commits => match &self.commit_files {
                // The diff of the file (or folder) of the commit being explored: `oid`, a separator, the path.
                Some(cf) => Some(format!("{}\u{1}{}", cf.oid, cf.current_path().unwrap_or(""))),
                None if self.work => self.commits.get(self.lists[1].cursor).map(|c| format!("{}\u{3}", c.oid)),
                None => self.commits.get(self.lists[1].cursor).map(|c| match &self.compare {
                    // Two commits: the diff from the marked one to this one.
                    Some(base) if *base != c.oid => format!("{base}\u{2}{}", c.oid),
                    _ => c.oid.clone(),
                }),
            },
            Focus::Stashes => self.stashes.get(self.lists[2].cursor).map(|s| format!("stash@{{{}}}", s.index)),
            Focus::Files => None,
        }
    }

    fn render_preview(&mut self, which: Focus, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let rev = self.current_rev(which);
        let short = |s: &str| s.chars().take(8).collect::<String>();
        let whole = rev.clone().unwrap_or_default();
        let title = if let Some(a) = whole.strip_suffix('\u{3}') {
            format!("{}..work tree", short(a))
        } else if let Some((a, b)) = whole.split_once('\u{2}') {
            format!("{}..{}", short(a), short(b))
        } else if let Some((_, p)) = whole.split_once('\u{1}') {
            if p.is_empty() { "all files".to_string() } else { p.to_string() }
        } else {
            whole
        };
        let icon = match which {
            Focus::Branches => ui::icons::BRANCH,
            Focus::Stashes => ui::icons::STASH,
            _ => ui::icons::COMMIT,
        };
        let pane = Pane::new(theme).title(&title).icon(icon);
        let inner = pane.inner(area);
        pane.render(area, buf);
        let Some(rev) = rev else {
            self.preview = None;
            return;
        };
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let key = (which, rev.clone(), inner.width);
        let mut same_rev = false;
        if self.preview.as_ref().map(|p| &p.key) != Some(&key) {
            // A different commit starts with the files folded the way they fold by themselves.
            same_rev = self.fold_rev.as_deref() == Some(rev.as_str());
            if !same_rev {
                pv::diff_reset_folds();
                self.fold_rev = Some(rev.clone());
            }
            let opts = pv::Options { width: Some(inner.width), height: None, git: false, focus: None };
            let result = match rev.split_once('\u{1}') {
                _ if rev.ends_with('\u{3}') => {
                    let base = rev.trim_end_matches('\u{3}');
                    match Repo::discover(&self.root).ok_or_else(|| "not a repository".to_string()).and_then(|r| r.worktree_snapshot().map_err(|e| e.to_string())) {
                        Ok(snap) => pv::preview_range_marked(&self.root, Some(base), &snap, Some("work tree"), theme, &opts),
                        Err(e) => Err(pv::Error::Unsupported(e)),
                    }
                }
                _ if rev.contains('\u{2}') => {
                    let (base, oid) = rev.split_once('\u{2}').unwrap_or_default();
                    pv::preview_range_marked(&self.root, Some(base), oid, None, theme, &opts)
                }
                Some((oid, path)) => pv::preview_commit_files_marked(&self.root, oid, path, theme, &opts),
                None => pv::preview_commit_marked(&self.root, &rev, theme, &opts),
            };
            let (lines, anchors) = match result {
                Ok(r) => r,
                Err(e) => (vec![Line::from(Span::styled(e.to_string(), Style::new().fg(theme.colors.muted.into())))], Default::default()),
            };
            self.preview = Some(Preview { key, lines, anchors });
            if !same_rev {
                self.pscroll = 0;
            }
        }
        self.pheight = usize::from(inner.height);
        if let Some(p) = &self.preview {
            self.pscroll = self.pscroll.min(p.lines.len().saturating_sub(1));
            let shown: Vec<Line<'static>> = p.lines.iter().skip(self.pscroll).take(usize::from(inner.height)).cloned().collect();
            Paragraph::new(shown).render(inner, buf);
        }
    }

    fn render_status(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let line = match &self.mode {
            Mode::Prompt(kind, input) => {
                let label = match kind {
                    PromptKind::Search => "search: ",
                    PromptKind::NewBranch => "new branch: ",
                    PromptKind::NewStash => "stash message: ",
                    PromptKind::RenameBranch(_) => "rename to: ",
                    PromptKind::RenameTag(_) => "rename tag to: ",
                    PromptKind::RenameRemote(_) => "rename remote to: ",
                    PromptKind::RenameRemoteBranch(_) => "rename on the remote to: ",
                    PromptKind::RenameFolder { .. } => "rename folder to: ",
                    PromptKind::BranchAt(_) => "new branch here: ",
                    PromptKind::TagAt(_) => "tag name: ",
                    PromptKind::Reword(_) => "message: ",
                    PromptKind::Amend(_) => "amend with message: ",
                    PromptKind::SwitchTo => "switch to (branch, tag or commit): ",
                };
                ui::prompt::cursor(buf, area, label, input.visual_cursor());
                if matches!(kind, PromptKind::SwitchTo) {
                    self.suggest.render(buf, area, theme);
                }
                ui::prompt::line(theme, label, input.value())
            }
            Mode::Confirm(cf) => {
                let q = match cf {
                    Confirm::DeleteBranch { name, force } => format!("{} branch {name}? (d again or y)", if *force { "Force-delete" } else { "Delete" }),
                    Confirm::Merge(name) => format!("Merge {name} into {}? (y/n)", self.files.branch()),
                    Confirm::DropStash(i) => format!("Drop stash@{{{i}}}? (d again or y)"),
                    Confirm::Rebase(name) => format!("Rebase {} onto {name}? (y/n)", self.files.branch()),
                    Confirm::Revert(_) => "Revert this commit? (y/n)".to_string(),
                    Confirm::Reset { .. } => "Reset hard to this commit and lose uncommitted changes? (y/n)".to_string(),
                    Confirm::ForcePush => format!("Force-push {} (with lease)? (y/n)", self.files.branch()),
                    Confirm::DeleteTag(name) => format!("Delete tag {name}? (d again or y)"),
                    Confirm::DeleteRemote(name) => format!("Delete {name} on the remote? (d again or y)"),
                };
                Line::from(Span::styled(q, Style::new().fg(c.warning.into())))
            }
            Mode::Popup(_) => Line::default(),
            Mode::Normal if self.job.is_some() => Line::from(Span::styled(format!("{} …", self.job.as_ref().map(|j| j.label.as_str()).unwrap_or("")), Style::new().fg(c.accent.into()))),
            Mode::Normal => match &self.message {
                Some((m, err)) => Line::from(Span::styled(m.clone(), Style::new().fg(if *err { c.error } else { c.accent }.into()))),
                None => match self.operation_name() {
                    Some(op) => Line::from(vec![
                        Span::styled(format!("{op} "), Style::new().fg(c.warning.into()).add_modifier(Modifier::BOLD)),
                        Span::styled("resolve the conflicts and stage them, then space o c continues, space o a aborts", Style::new().fg(c.muted.into())),
                    ]),
                    None => Line::from(Span::styled(self.hints(), Style::new().fg(c.muted.into()))),
                },
            },
        };
        Paragraph::new(line).render(Rect { x: area.x + 1, width: area.width.saturating_sub(2), ..area }, buf);
    }

    fn hints(&self) -> &'static str {
        if self.screen == Screen::Commits {
            if self.commit_files.is_some() {
                return "j k move  l h fold  ctrl-d/u scroll diff  } { hunks  ) ( files  esc back to the commits";
            }
            return "/ search  n N next  f filter  F clear  v view  l files  y copy hash  enter checkout  C cherry-pick  [ ] tabs  ? keys";
        }
        match self.focus {
            Focus::Files => "s stage/unstage  a all  d discard  t tree/list  c commit  enter edit  [ ] tabs  space space commands  ? keys",
            Focus::Branches => "enter checkout  n new  d delete  M merge  B rebase  tab stashes  space r fetch/pull/push  [ ] tabs  ? keys",
            Focus::Commits => "enter checkout  m compare  M vs work tree  C cherry-pick  R revert  r reword  A amend  b branch  t tag  space z reset  space space commands  ? keys",
            Focus::Stashes => "n new  a apply  p pop  d drop  tab refs  [ ] tabs  ? keys",
        }
    }
}

/// Width of the hash column of the Commits table.
const HASH_W: usize = 8;

/// The widest an author name column gets.
const AUTHOR_COL: usize = 18;

/// Cells a ref takes as a pill (the caps, the name, the branch icon of the checked-out one, a space).
fn ref_width(r: &RefInfo, remotes: &[String]) -> usize {
    pill_text(r, remotes).chars().count() + 3
}

/// The text of a pill: the name, with the remotes that point at the same commit under the same name
/// (`main · origin`), and the branch icon on the checked-out branch.
fn pill_text(r: &RefInfo, remotes: &[String]) -> String {
    let mut text = if r.is_head { format!("\u{e0a0} {}", r.name) } else { r.name.clone() };
    if !remotes.is_empty() {
        text.push_str(&format!(" · {}", remotes.join(", ")));
    }
    text
}

/// The refs at one commit as pills: a branch and its remote branches (`origin/main`) share a pill.
fn pills(refs: &[RefInfo]) -> Vec<(RefInfo, Vec<String>)> {
    let mut out: Vec<(RefInfo, Vec<String>)> = Vec::new();
    let mut used = vec![false; refs.len()];
    for (i, r) in refs.iter().enumerate() {
        if r.kind == RefKind::Remote {
            continue;
        }
        let suffix = format!("/{}", r.name);
        let mut remotes = Vec::new();
        if r.kind == RefKind::Branch {
            for (j, o) in refs.iter().enumerate() {
                if o.kind == RefKind::Remote && o.name.ends_with(&suffix) {
                    remotes.push(o.name[..o.name.len() - suffix.len()].to_string());
                    used[j] = true;
                }
            }
        }
        used[i] = true;
        out.push((r.clone(), remotes));
    }
    for (j, o) in refs.iter().enumerate() {
        if !used[j] {
            out.push((o.clone(), Vec::new()));
        }
    }
    out
}

/// A branch or tag name as a pill with rounded ends. The checked-out branch is solid with the
/// branch icon; the others are a tint of their colour (branches blue, remotes grey, tags yellow).
fn ref_pill(theme: &Theme, r: &RefInfo, remotes: &[String]) -> Vec<Span<'static>> {
    use ratatui::style::Color;
    let c = &theme.colors;
    let color = match (r.is_head, r.kind) {
        (true, _) => Color::from(c.success),
        (_, RefKind::Branch) => Color::from(c.info),
        (_, RefKind::Remote) => Color::from(c.muted),
        (_, RefKind::Tag) => Color::from(c.warning),
    };
    let (bg, fg, text) = if r.is_head {
        (color, Color::from(c.bg), pill_text(r, remotes))
    } else {
        (ui::mix(color, Color::from(c.bg), 30), color, pill_text(r, remotes))
    };
    let mut body = Style::new().fg(fg).bg(bg);
    if r.is_head {
        body = body.add_modifier(Modifier::BOLD);
    }
    let cap = Style::new().fg(bg);
    vec![Span::styled("\u{e0b6}", cap), Span::styled(text, body), Span::styled("\u{e0b4}", cap), Span::raw(" ")]
}

/// The author as two letters: the first letters of the first two words (`Chetan Singh Sajwan` ->
/// `CS`), or the first two letters of a single name (`root` -> `ro`).
fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let letters: Vec<char> = match words.as_slice() {
        [] => vec!['?'],
        [one] => one.chars().take(2).collect(),
        [a, b, ..] => a.chars().take(1).chain(b.chars().take(1)).collect(),
    };
    letters.into_iter().collect()
}

/// A stable color per author, from the theme's accents, so one person always looks the same.
fn author_color(theme: &Theme, name: &str) -> ratatui::style::Color {
    let c = &theme.colors;
    let palette = [c.success, c.warning, c.info, c.accent, c.git.untracked, c.git.renamed, c.git.modified, c.git.conflict];
    let hash = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33).wrapping_add(u32::from(b)));
    palette[hash as usize % palette.len()].into()
}

/// `now`, `5m ago`, `4h ago`, ... for a commit time in seconds since the epoch.
fn ago(time: i64) -> String {
    ui::time::ago(time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;
    use std::fs;
    use std::process::Command;
    use std::sync::Once;

    fn identity() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            for (k, v) in [("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t"), ("GIT_CONFIG_GLOBAL", "/dev/null"), ("GIT_CONFIG_SYSTEM", "/dev/null")] {
                // SAFETY: set once, before any test starts git, to constants.
                unsafe { std::env::set_var(k, v) };
            }
        });
    }

    fn git(dir: &Path, args: &[&str]) {
        assert!(Command::new("git").args(args).current_dir(dir).status().unwrap().success(), "git {args:?}");
    }

    fn repo(name: &str) -> PathBuf {
        identity();
        let d = std::env::temp_dir().join(format!("git-ui-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        git(&d, &["init", "-q", "-b", "main"]);
        fs::write(d.join("a.txt"), "1\n2\n").unwrap();
        git(&d, &["add", "."]);
        git(&d, &["commit", "-qm", "first commit"]);
        git(&d, &["branch", "other"]);
        d.canonicalize().unwrap()
    }

    /// The row of the Refs tree that shows the ref called `name`.
    fn ref_row(app: &App, name: &str) -> usize {
        app.ref_rows.iter().position(|r| matches!(r, RefRow::Item { index, .. } if app.branches[*index].name == name)).unwrap_or_else(|| panic!("no ref {name}"))
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn press(app: &mut App, keys: &str) {
        for c in keys.chars() {
            assert_eq!(app.handle_event(&key(KeyCode::Char(c))), Quit::No);
        }
    }

    fn screen(app: &mut App) -> Vec<String> {
        let area = Rect::new(0, 0, 120, 36);
        let mut buf = Buffer::empty(area);
        app.render(area, &mut buf);
        (0..36).map(|y| (0..120).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string()).collect()
    }

    fn head(d: &Path) -> String {
        Repo::discover(d).unwrap().branch().unwrap()
    }

    #[test]
    fn the_commits_tab_shows_every_branch_and_searches() {
        let d = repo("tab");
        git(&d, &["checkout", "-q", "-b", "side"]);
        fs::write(d.join("b.txt"), "b\n").unwrap();
        git(&d, &["add", "."]);
        git(&d, &["commit", "-qm", "needle on a side branch"]);
        git(&d, &["checkout", "-q", "main"]);
        let mut app = App::new(&d).unwrap();
        // The Status tab lists the checked-out branch only.
        assert!(!screen(&mut app).join("\n").contains("needle"));
        // `]` opens the Commits tab: every branch, with the graph and the ref labels.
        press(&mut app, "]");
        let all = screen(&mut app).join("\n");
        assert!(all.contains("needle on a side branch") && all.contains("side") && all.contains("●"), "{all}");
        // `/` searches: the cursor lands on the match and the message counts it.
        press(&mut app, "/");
        type_text(&mut app, "needle");
        assert_eq!(app.commits[app.lists[1].cursor].subject, "needle on a side branch");
        assert!(app.message.as_ref().is_some_and(|(m, _)| m.contains("match 1 of 1")), "{:?}", app.message);
        // Esc clears the search first, then leaves the tab and restores the pane's scope.
        app.handle_event(&key(KeyCode::Esc));
        assert!(app.search.is_none());
        press(&mut app, "[");
        assert!(!screen(&mut app).join("\n").contains("needle"));
    }

    #[test]
    fn switch_to_takes_a_ref_a_chosen_option_or_a_commit_hash() {
        let d = repo("switch");
        git(&d, &["branch", "topic-one"]);
        git(&d, &["branch", "side-two"]);
        let mut app = App::new(&d).unwrap();
        let root = app.repo.commits(5).unwrap()[0].oid.clone();
        press(&mut app, " ");
        press(&mut app, "S");
        type_text(&mut app, "topic-one");
        assert_eq!(app.repo.branch().as_deref(), Some("topic-one"));
        // A fragment picks the first match.
        press(&mut app, " ");
        press(&mut app, "S");
        type_text(&mut app, "side");
        assert_eq!(app.repo.branch().as_deref(), Some("side-two"));
        // A hash detaches HEAD.
        press(&mut app, " ");
        press(&mut app, "S");
        type_text(&mut app, &root[..8]);
        assert_eq!(app.repo.branch(), None, "{:?}", app.message);
        // Nothing of that name.
        press(&mut app, " ");
        press(&mut app, "S");
        type_text(&mut app, "zzzz-nothing");
        assert!(app.message.as_ref().is_some_and(|(m, err)| *err && m.contains("no branch")));
    }

    #[test]
    fn amend_adds_the_staged_file_and_enter_keeps_the_message() {
        let d = repo("amend");
        let mut app = App::new(&d).unwrap();
        fs::write(d.join("more.txt"), "x\n").unwrap();
        git(&d, &["add", "more.txt"]);
        let before = app.repo.commits(5).unwrap().len();
        press(&mut app, "2");
        press(&mut app, "A");
        app.handle_event(&key(KeyCode::Enter));
        assert_eq!(app.repo.commits(5).unwrap().len(), before, "amended, not added");
        assert!(app.repo.status().unwrap().is_empty(), "{:?}", app.message);
    }

    fn tab(app: &mut App) {
        app.handle_event(&key(KeyCode::Tab));
    }

    #[test]
    fn l_on_a_commit_shows_its_files_as_a_tree_with_the_diff_of_each() {
        let d = repo("cfiles");
        fs::create_dir_all(d.join("src/deep")).unwrap();
        fs::write(d.join("src/a.rs"), "fn a() {}\n").unwrap();
        fs::write(d.join("src/deep/b.rs"), "fn b() {}\n").unwrap();
        fs::write(d.join("top.txt"), "t\n").unwrap();
        git(&d, &["add", "."]);
        git(&d, &["commit", "-q", "-m", "add files"]);
        let mut app = App::new(&d).unwrap();
        press(&mut app, "2");
        press(&mut app, "l");
        let text = screen(&mut app).join("\n");
        assert!(text.contains("add files") && text.contains("src/") && text.contains("top.txt") && text.contains("+1"), "{text}");
        // The diff of the file under the cursor shows beside the tree.
        let cf = app.commit_files.as_ref().unwrap();
        let at = cf.rows.iter().position(|r| matches!(r, FileRow::File { change, .. } if change.path == "src/a.rs")).unwrap();
        app.commit_files.as_mut().unwrap().cursor = at;
        let text = screen(&mut app).join("\n");
        assert!(text.contains("fn a()"), "{text}");
        // h folds and unfolds a folder, and goes to the parent; esc goes back to the commits.
        app.commit_files.as_mut().unwrap().cursor = 0;
        let open = app.commit_files.as_ref().unwrap().rows.len();
        press(&mut app, "h");
        assert!(app.commit_files.as_ref().unwrap().rows.len() < open, "the folder is folded");
        press(&mut app, "l");
        assert_eq!(app.commit_files.as_ref().unwrap().rows.len(), open);
        app.handle_event(&key(KeyCode::Esc));
        assert!(app.commit_files.is_none());
        assert!(screen(&mut app).join("\n").contains("add files"));
    }

    #[test]
    fn the_filter_popup_chooses_several_things_and_the_bar_shows_them() {
        let d = repo("filter");
        commit_file(&d, "b.txt", "b\n", "add feature alpha");
        commit_file(&d, "c.txt", "c\n", "fix: bug beta");
        commit_file(&d, "d.txt", "d\n", "feat(ui): gamma");
        let mut app = App::new(&d).unwrap();
        press(&mut app, "2");
        assert_eq!(app.commits.len(), 4);
        // By message: the text section is the first.
        press(&mut app, "f");
        assert!(screen(&mut app).join("\n").contains("Filter commits"));
        type_text(&mut app, "alpha");
        assert_eq!(app.commits.len(), 1, "{:?}", app.message);
        assert!(screen(&mut app).join("\n").contains("message alpha"), "the bar names the filter");
        press(&mut app, "F");
        assert_eq!(app.commits.len(), 4);
        // By category: feat, then feat and fix together.
        press(&mut app, "f");
        tab(&mut app); // authors
        tab(&mut app); // categories: feat, fix
        press(&mut app, " ");
        app.handle_event(&key(KeyCode::Enter));
        assert_eq!(app.commits.len(), 1, "{:?}", app.message);
        assert_eq!(app.commits[0].subject, "feat(ui): gamma");
        press(&mut app, "f");
        tab(&mut app);
        tab(&mut app);
        app.handle_event(&key(KeyCode::Down));
        press(&mut app, " ");
        app.handle_event(&key(KeyCode::Enter));
        assert_eq!(app.commits.len(), 2);
        assert!(screen(&mut app).join("\n").contains("category feat, fix"));
        // The popup shows what is set, and a date that excludes everything empties the list.
        press(&mut app, "f");
        for _ in 0..4 {
            tab(&mut app);
        }
        press(&mut app, " ");
        assert!(screen(&mut app).join("\n").contains("since: 1d"));
        app.handle_event(&key(KeyCode::Enter));
        assert_eq!(app.commits.len(), 2, "everything is newer than a day");
        app.filter.since = Some(("x".into(), i64::MAX));
        app.refilter("x");
        assert_eq!(app.commits.len(), 0);
    }

    #[test]
    fn since_understands_units_and_dates() {
        let now = 1_000_000_000;
        assert_eq!(parse_since("7d", now), Some(now - 7 * 86_400));
        assert_eq!(parse_since("2w", now), Some(now - 14 * 86_400));
        assert_eq!(parse_since("1970-01-02", now), Some(86_400));
        assert_eq!(parse_since("2000-03-01", now), Some(951_868_800));
        assert_eq!(parse_since("tomorrow", now), None);
        assert_eq!(parse_since("2026-13-01", now), None);
    }

    #[test]
    fn shows_the_files_page_and_the_commits_and_refs_tabs() {
        let d = repo("panes");
        let mut app = App::new(&d).unwrap();
        // The first page is the changed files only, under the tab strip.
        let all = screen(&mut app).join("\n");
        for want in ["Status", "Commits", "Refs", "Files"] {
            assert!(all.contains(want), "{want} in\n{all}");
        }
        assert!(!all.contains("Stashes") && !all.contains("Branches"), "{all}");
        // Refs: the branches (and tags, remotes) and the stashes.
        press(&mut app, "3");
        let all = screen(&mut app).join("\n");
        assert!(all.contains("Stashes") && all.contains("main") && all.contains("other"), "{all}");
        // Commits: the graph with the refs as pills.
        press(&mut app, "2");
        let all = screen(&mut app).join("\n");
        assert!(all.contains("●") && all.contains("main") && all.contains("other"), "{all}");
    }

    #[test]
    fn focus_moves_with_tab_and_digits_and_q_quits() {
        let d = repo("focus");
        let mut app = App::new(&d).unwrap();
        press(&mut app, "2");
        assert_eq!((app.screen, app.focus), (Screen::Commits, Focus::Commits));
        // Commits has one pane: Tab stays. Refs has two: Tab and Shift-Tab move between them.
        app.handle_event(&key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Commits);
        press(&mut app, "3");
        assert_eq!((app.screen, app.focus), (Screen::Refs, Focus::Branches));
        app.handle_event(&key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Stashes);
        app.handle_event(&key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Branches);
        app.handle_event(&Event::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)));
        assert_eq!(app.focus, Focus::Stashes);
        press(&mut app, "1");
        assert_eq!((app.screen, app.focus), (Screen::Status, Focus::Files));
        assert_eq!(app.handle_event(&key(KeyCode::Char('q'))), Quit::Yes);
    }

    #[test]
    fn the_refs_cursor_follows_its_ref_across_reloads_and_case_makes_folders() {
        let d = repo("cursor");
        git(&d, &["branch", "Feature/a"]);
        git(&d, &["branch", "feature/b"]);
        git(&d, &["branch", "Feature/c"]);
        let mut app = App::new(&d).unwrap();
        press(&mut app, "3");
        // `Feature/` and `feature/` are two folders, each once.
        let folders: Vec<String> = app.ref_rows.iter().filter_map(|r| if let RefRow::Folder { name, .. } = r { Some(name.clone()) } else { None }).collect();
        assert_eq!(folders, vec!["Feature".to_string(), "feature".to_string()]);
        // A new branch sorts before the one under the cursor: the cursor stays on its ref.
        app.lists[0].cursor = ref_row(&app, "other");
        git(&d, &["branch", "aaa"]);
        app.reload_all();
        assert_eq!(app.current_ref().map(|b| b.name.as_str()), Some("other"));
        // A folder rename that cannot be done as a whole does nothing at all.
        git(&d, &["branch", "taken-a"]);
        app.reload_all();
        app.lists[0].cursor = app.ref_rows.iter().position(|r| matches!(r, RefRow::Folder { name, .. } if name == "Feature")).unwrap();
        press(&mut app, "r");
        for _ in 0..7 {
            app.handle_event(&key(KeyCode::Backspace));
        }
        type_text(&mut app, "x");
        assert!(app.branches.iter().any(|b| b.name == "x/a") && app.branches.iter().any(|b| b.name == "x/c"), "{:?}", app.message);
        let before: Vec<String> = app.branches.iter().map(|b| b.name.clone()).collect();
        app.lists[0].cursor = app.ref_rows.iter().position(|r| matches!(r, RefRow::Folder { name, .. } if name == "x")).unwrap();
        press(&mut app, "r");
        for _ in 0..1 {
            app.handle_event(&key(KeyCode::Backspace));
        }
        type_text(&mut app, "feature"); // `feature/b` exists, but x/a -> feature/a and x/c -> feature/c are free
        let after: Vec<String> = app.branches.iter().map(|b| b.name.clone()).collect();
        assert_ne!(before, after);
        // A name that is not valid says so and changes nothing.
        let snapshot = after.clone();
        app.lists[0].cursor = app.ref_rows.iter().position(|r| matches!(r, RefRow::Folder { name, .. } if name == "feature")).unwrap();
        press(&mut app, "r");
        for _ in 0..7 {
            app.handle_event(&key(KeyCode::Backspace));
        }
        type_text(&mut app, "bad name");
        assert!(app.message.as_ref().is_some_and(|(_, e)| *e), "{:?}", app.message);
        assert_eq!(app.branches.iter().map(|b| b.name.clone()).collect::<Vec<_>>(), snapshot);
    }

    #[test]
    fn tags_and_folders_of_refs_can_be_renamed() {
        let d = repo("renames");
        git(&d, &["branch", "feature/login"]);
        git(&d, &["branch", "feature/signup"]);
        git(&d, &["tag", "v1"]);
        let mut app = App::new(&d).unwrap();
        press(&mut app, "3");
        // A tag.
        app.lists[0].cursor = ref_row(&app, "v1");
        press(&mut app, "r");
        for _ in 0..2 {
            app.handle_event(&key(KeyCode::Backspace));
        }
        type_text(&mut app, "v2");
        assert!(app.branches.iter().any(|b| b.name == "v2") && app.branches.iter().all(|b| b.name != "v1"), "{:?}", app.message);
        // A folder: everything under `feature/` moves to `feat/`.
        app.lists[0].cursor = app.ref_rows.iter().position(|r| matches!(r, RefRow::Folder { name, .. } if name == "feature")).unwrap();
        press(&mut app, "r");
        for _ in 0..7 {
            app.handle_event(&key(KeyCode::Backspace));
        }
        type_text(&mut app, "feat");
        let names: Vec<&str> = app.branches.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"feat/login") && names.contains(&"feat/signup") && !names.iter().any(|n| n.starts_with("feature/")), "{names:?} {:?}", app.message);
        // A section cannot be renamed.
        app.lists[0].cursor = 0;
        press(&mut app, "r");
        assert!(matches!(app.mode, Mode::Normal) && app.message.as_ref().is_some_and(|(_, e)| *e));
    }

    #[test]
    fn the_refs_are_a_tree_of_sections_and_folders_that_fold() {
        let d = repo("reftree");
        git(&d, &["branch", "feature/login"]);
        git(&d, &["branch", "feature/signup/form"]);
        git(&d, &["tag", "v1"]);
        let mut app = App::new(&d).unwrap();
        let shape = |app: &App| -> Vec<String> {
            app.ref_rows
                .iter()
                .map(|r| match r {
                    RefRow::Section { label, .. } => label.to_string(),
                    RefRow::Folder { name, depth, .. } => format!("{}{name}/", " ".repeat(*depth)),
                    RefRow::Item { name, depth, .. } => format!("{}{name}", " ".repeat(*depth)),
                })
                .collect()
        };
        assert_eq!(shape(&app), vec!["Branches", " feature/", "  login", "  signup/", "   form", " main", " other", "Tags", " v1"]);
        // Enter on a folder folds it; the cursor stays on the folder.
        press(&mut app, "3");
        app.lists[0].cursor = app.ref_rows.iter().position(|r| matches!(r, RefRow::Folder { name, .. } if name == "feature")).unwrap();
        app.handle_event(&key(KeyCode::Enter));
        assert_eq!(shape(&app), vec!["Branches", " feature/", " main", " other", "Tags", " v1"]);
        assert!(matches!(app.ref_rows[app.lists[0].cursor], RefRow::Folder { .. }));
        // Enter again unfolds it, a section folds all its refs.
        app.handle_event(&key(KeyCode::Enter));
        assert_eq!(app.ref_rows.len(), 9);
        app.lists[0].cursor = 0;
        app.handle_event(&key(KeyCode::Enter));
        assert_eq!(shape(&app), vec!["Branches", "Tags", " v1"]);
    }

    #[test]
    fn branches_checkout_create_and_delete() {
        let d = repo("branches");
        let mut app = App::new(&d).unwrap();
        press(&mut app, "3");
        // main (checked out) is first, `other` second: j then Enter switches.
        press(&mut app, "j");
        app.handle_event(&key(KeyCode::Enter));
        assert_eq!(head(&d), "other");
        // New branch through the prompt.
        press(&mut app, "n");
        press(&mut app, "topic");
        app.handle_event(&key(KeyCode::Enter));
        assert_eq!(head(&d), "topic");
        // Delete `other` (not checked out) with a confirmation.
        app.lists[0].cursor = ref_row(&app, "other");
        press(&mut app, "d");
        assert!(matches!(app.mode, Mode::Confirm(_)));
        press(&mut app, "y");
        assert!(app.branches.iter().all(|b| b.name != "other"), "{:?}", app.branches);
        // The checked-out branch cannot be deleted.
        app.lists[0].cursor = ref_row(&app, "topic");
        press(&mut app, "d");
        assert!(matches!(app.mode, Mode::Normal) && app.message.as_ref().is_some_and(|(_, e)| *e));
    }

    #[test]
    fn stash_new_apply_pop_and_drop() {
        let d = repo("stash");
        let mut app = App::new(&d).unwrap();
        fs::write(d.join("a.txt"), "changed\n").unwrap();
        press(&mut app, "4");
        press(&mut app, "n");
        press(&mut app, "wip");
        app.handle_event(&key(KeyCode::Enter));
        assert_eq!(app.stashes.len(), 1, "{:?}", app.message);
        assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "1\n2\n");
        press(&mut app, "p");
        assert!(app.stashes.is_empty());
        assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "changed\n");
        // Drop asks first.
        press(&mut app, "n");
        press(&mut app, "again");
        app.handle_event(&key(KeyCode::Enter));
        assert_eq!(app.stashes.len(), 1);
        press(&mut app, "d");
        press(&mut app, "n"); // anything but y cancels
        assert_eq!(app.stashes.len(), 1);
        press(&mut app, "d");
        press(&mut app, "y");
        assert!(app.stashes.is_empty());
    }

    #[test]
    fn a_branch_and_its_remote_branch_share_a_pill() {
        let r = |name: &str, kind| RefInfo { name: name.into(), kind, oid: "x".into(), is_head: false, subject: String::new(), time: 0 };
        let refs = [r("main", RefKind::Branch), r("origin/main", RefKind::Remote), r("origin/other", RefKind::Remote), r("v1", RefKind::Tag)];
        let got: Vec<(String, Vec<String>)> = pills(&refs).into_iter().map(|(r, o)| (r.name, o)).collect();
        assert_eq!(got, vec![("main".to_string(), vec!["origin".to_string()]), ("v1".to_string(), vec![]), ("origin/other".to_string(), vec![])]);
        assert_eq!(pill_text(&refs[0], &["origin".to_string()]), "main · origin");
    }

    #[test]
    fn initials_and_author_colors() {
        assert_eq!(initials("Chetan Singh Sajwan"), "CS");
        assert_eq!(initials("root"), "ro");
        assert_eq!(initials("Ada"), "Ad");
        assert_eq!(initials("  "), "?");
        let t = Theme::default();
        assert_eq!(author_color(&t, "Ada Lovelace"), author_color(&t, "Ada Lovelace"));
    }

    #[test]
    fn the_files_pane_still_stages_and_commits() {
        let d = repo("files");
        let mut app = App::new(&d).unwrap();
        fs::write(d.join("a.txt"), "1\n2\n3\n").unwrap();
        app.reload_all();
        press(&mut app, "s"); // stage the highlighted file
        press(&mut app, "c");
        press(&mut app, "second");
        app.handle_event(&key(KeyCode::Enter));
        press(&mut app, "2"); // the commits load when their tab is shown
        assert_eq!(app.commits.len(), 2, "{:?}", app.message);
        assert_eq!(app.commits[0].subject, "second");
    }

    #[test]
    fn the_leader_menu_palette_and_help_work_in_every_pane() {
        let d = repo("leader");
        let mut app = App::new(&d).unwrap();
        // Files pane: its own table; Space then Space asks the host for the palette.
        press_raw(&mut app, ' ');
        assert!(app.files.menu().is_some());
        assert_eq!(app.handle_event(&key(KeyCode::Char(' '))), Quit::Palette);
        let ids: Vec<&str> = app.keys_rows().iter().map(|r| r.id).collect();
        assert!(ids.contains(&"commit.new") && ids.contains(&"focus.branches"), "{ids:?}");
        // Branches pane: the leader menu lists the branch actions, a leader key runs one.
        press(&mut app, "3");
        press_raw(&mut app, ' ');
        let menu = match app.focus { Focus::Branches => app.maps[0].menu().unwrap(), _ => panic!() };
        let names: Vec<&str> = menu.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"new branch") && names.contains(&"merge") && names.contains(&"commands"), "{names:?}");
        press_raw(&mut app, 'n');
        assert!(matches!(app.mode, Mode::Prompt(PromptKind::NewBranch, _)));
        app.handle_event(&key(KeyCode::Esc));
        // The palette rows of a pane hold the basic keys too (they are filtered when shown).
        let rows = app.keys_rows();
        assert!(rows.iter().any(|r| r.id == "nav.down" && r.basic) && rows.iter().any(|r| r.id == "branch.merge" && !r.basic));
        // The palette's pick runs in the pane; `?` opens the help, any other key closes it with Esc.
        assert_eq!(app.run_palette("branch.new"), Quit::No);
        assert!(matches!(app.mode, Mode::Prompt(PromptKind::NewBranch, _)));
        app.handle_event(&key(KeyCode::Esc));
        press(&mut app, "?");
        assert!(app.help.is_some());
        app.handle_event(&key(KeyCode::Esc));
        assert!(app.help.is_none());
        // Stash pane has its own actions.
        press(&mut app, "4");
        let ids: Vec<&str> = app.keys_rows().iter().map(|r| r.id).collect();
        assert!(ids.contains(&"stash.apply") && !ids.contains(&"branch.merge"), "{ids:?}");
    }

    fn press_raw(app: &mut App, c: char) {
        app.handle_event(&key(KeyCode::Char(c)));
    }

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle_event(&key(KeyCode::Char(c)));
        }
        app.handle_event(&key(KeyCode::Enter));
    }

    fn commit_file(d: &Path, name: &str, body: &str, msg: &str) {
        fs::write(d.join(name), body).unwrap();
        git(d, &["add", name]);
        git(d, &["commit", "-qm", msg]);
    }

    #[test]
    fn capital_m_shows_the_diff_from_a_commit_to_the_work_tree() {
        let d = repo("workdiff");
        commit_file(&d, "b.txt", "two\n", "second");
        fs::write(d.join("b.txt"), "two changed\n").unwrap();
        let mut app = App::new(&d).unwrap();
        press(&mut app, "2");
        press(&mut app, "G");
        press(&mut app, "M");
        let text = screen(&mut app).join("\n");
        assert!(text.contains("work tree") && text.contains("b.txt") && text.contains("two changed"), "{text}");
        press(&mut app, "M");
        assert!(!app.work);
    }

    #[test]
    fn the_refs_column_fits_the_widest_commit_and_the_free_space_goes_to_what_is_left() {
        let d = repo("cols");
        commit_file(&d, "b.txt", "two\n", "second");
        git(&d, &["tag", "a-rather-long-tag-name"]);
        git(&d, &["tag", "another-long-tag-name"]);
        let mut app = App::new(&d).unwrap();
        press(&mut app, "2");
        app.fit_columns(120);
        assert!(app.refs_w > 24, "the refs of the widest commit all fit: {}", app.refs_w);
        // Without the message the refs take the rest of the row.
        app.cols.subject = false;
        app.fit_columns(120);
        assert!(app.refs_w > app.refs_need, "{} vs {}", app.refs_w, app.refs_need);
        // In a narrow list the message keeps its room and the refs give way.
        app.cols.subject = true;
        app.fit_columns(60);
        assert!(app.refs_w <= 60 - 24, "{}", app.refs_w);
    }

    #[test]
    fn z_folds_the_file_at_the_top_of_the_diff_and_w_ignores_whitespace() {
        let d = repo("fold");
        commit_file(&d, "b.txt", "two\n", "second");
        let mut app = App::new(&d).unwrap();
        press(&mut app, "2");
        press(&mut app, "p");
        let before = screen(&mut app).join("\n");
        assert!(before.contains("⋯ line"), "{before}");
        press(&mut app, "z");
        let folded = screen(&mut app).join("\n");
        assert!(folded.contains("folded"), "{folded}");
        press(&mut app, "z");
        assert!(!screen(&mut app).join("\n").contains("folded"));
        press(&mut app, "w");
        assert!(pv::diff_ignoring_whitespace());
        press(&mut app, "w");
        assert!(!pv::diff_ignoring_whitespace());
    }

    #[test]
    fn m_marks_a_commit_and_the_preview_shows_the_diff_to_the_highlighted_one() {
        let d = repo("compare");
        commit_file(&d, "b.txt", "two\n", "second");
        commit_file(&d, "c.txt", "three\n", "third");
        let mut app = App::new(&d).unwrap();
        press(&mut app, "2");
        // Newest first: mark the oldest, then move back to the newest.
        press(&mut app, "G");
        press(&mut app, "m");
        press(&mut app, "g");
        let text = screen(&mut app).join("\n");
        assert!(text.contains("from") && text.contains("to") && text.contains("b.txt") && text.contains("c.txt"), "{text}");
        // m on the marked commit, or Esc, clears it.
        press(&mut app, "G");
        press(&mut app, "m");
        assert!(app.compare.is_none());
    }

    #[test]
    fn commit_actions_check_out_tag_branch_reword_reset_and_revert() {
        let d = repo("commits");
        commit_file(&d, "b.txt", "two\n", "second");
        let mut app = App::new(&d).unwrap();
        press(&mut app, "2");
        // Reword the last commit.
        press(&mut app, "r");
        for _ in 0..6 {
            app.handle_event(&key(KeyCode::Backspace));
        }
        type_text(&mut app, "reworded");
        assert_eq!(app.commits[0].subject, "reworded", "{:?}", app.message);
        // Tag and branch at the first commit.
        press(&mut app, "j");
        press(&mut app, "t");
        type_text(&mut app, "v0");
        assert!(app.branches.iter().any(|b| b.name == "v0"), "{:?}", app.message);
        press(&mut app, "b");
        type_text(&mut app, "from-first");
        assert_eq!(app.repo.branch().as_deref(), Some("from-first"));
        // Check out the head commit of main again (detached), then cherry-pick the reworded one elsewhere.
        press(&mut app, "k");
        app.handle_event(&key(KeyCode::Enter));
        assert_eq!(app.repo.branch(), None, "a detached HEAD");
        git(&d, &["switch", "-q", "main"]);
        app.reload_all();
        // Revert asks first; reset hard asks first, soft does not.
        press(&mut app, "R");
        assert!(matches!(app.mode, Mode::Confirm(Confirm::Revert(_))));
        press(&mut app, "y");
        assert_eq!(app.commits.len(), 3, "{:?}", app.message);
        app.lists[1].cursor = 2;
        press(&mut app, " ");
        press(&mut app, "z");
        press(&mut app, "h");
        assert!(matches!(app.mode, Mode::Confirm(Confirm::Reset { .. })));
        press(&mut app, "y");
        assert_eq!(app.commits.len(), 1, "{:?}", app.message);
    }

    #[test]
    fn branches_rebase_rename_and_delete_tags() {
        let d = repo("branchops");
        git(&d, &["switch", "-q", "-c", "feat"]);
        commit_file(&d, "f.txt", "f\n", "feature");
        git(&d, &["switch", "-q", "main"]);
        commit_file(&d, "m.txt", "m\n", "main moves");
        git(&d, &["tag", "v9"]);
        git(&d, &["switch", "-q", "feat"]);
        let mut app = App::new(&d).unwrap();
        press(&mut app, "3");
        let at = |app: &App, name: &str| ref_row(app, name);
        // Rebase feat onto main.
        app.lists[0].cursor = at(&app, "main");
        press(&mut app, "B");
        assert!(matches!(app.mode, Mode::Confirm(Confirm::Rebase(_))));
        press(&mut app, "y");
        assert!(d.join("m.txt").exists() && d.join("f.txt").exists(), "{:?}", app.message);
        // Rename it.
        app.lists[0].cursor = at(&app, "feat");
        press(&mut app, "r");
        for _ in 0..4 {
            app.handle_event(&key(KeyCode::Backspace));
        }
        type_text(&mut app, "topic");
        assert!(app.branches.iter().any(|b| b.name == "topic"), "{:?}", app.message);
        // A tag is deleted through the same key, with a confirmation.
        app.lists[0].cursor = at(&app, "v9");
        press(&mut app, "d");
        assert!(matches!(app.mode, Mode::Confirm(Confirm::DeleteTag(_))));
        press(&mut app, "y");
        assert!(app.branches.iter().all(|b| b.name != "v9"));
    }

    #[test]
    fn remote_commands_run_in_the_background_and_report() {
        let d = repo("remote");
        let remote = std::env::temp_dir().join(format!("git-ui-remote-{}", std::process::id()));
        let _ = fs::remove_dir_all(&remote);
        fs::create_dir_all(&remote).unwrap();
        git(&remote, &["init", "-q", "--bare", "-b", "main"]);
        git(&d, &["remote", "add", "origin", remote.to_str().unwrap()]);
        let mut app = App::new(&d).unwrap();
        let wait = |app: &mut App| {
            for _ in 0..500 {
                if !app.tick() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        };
        // Space r P pushes (the first push sets the upstream) without blocking the screen.
        press_raw(&mut app, ' ');
        press_raw(&mut app, 'r');
        press_raw(&mut app, 'P');
        assert!(app.job.is_some());
        wait(&mut app);
        assert_eq!(app.message.as_ref().map(|(m, e)| (m.as_str(), *e)), Some(("pushed", false)), "{:?}", app.message);
        assert!(app.repo.has_upstream());
        // Fetch and pull report too; force push asks first.
        for k in ['f', 'p'] {
            press_raw(&mut app, ' ');
            press_raw(&mut app, 'r');
            press_raw(&mut app, k);
            wait(&mut app);
        }
        assert_eq!(app.message.as_ref().map(|(m, _)| m.as_str()), Some("pulled"));
        press_raw(&mut app, ' ');
        press_raw(&mut app, 'r');
        press_raw(&mut app, 'F');
        assert!(matches!(app.mode, Mode::Confirm(Confirm::ForcePush)));
        press(&mut app, "n");
        assert!(app.job.is_none());
    }

    #[test]
    fn a_stopped_operation_shows_in_the_status_line_and_can_be_aborted() {
        let d = repo("stopped");
        git(&d, &["switch", "-q", "other"]);
        commit_file(&d, "b.txt", "other\n", "other");
        git(&d, &["switch", "-q", "main"]);
        commit_file(&d, "b.txt", "main\n", "main");
        let mut app = App::new(&d).unwrap();
        press(&mut app, "3");
        app.lists[0].cursor = ref_row(&app, "other");
        press(&mut app, "M");
        press(&mut app, "y"); // the merge conflicts
        assert_eq!(app.operation_name(), Some("MERGING"), "{:?}", app.message);
        press(&mut app, "j"); // clears the failure message the merge left
        let shown = screen(&mut app).join("\n");
        assert!(shown.contains("MERGING"), "{shown}");
        press_raw(&mut app, ' ');
        press_raw(&mut app, 'o');
        press_raw(&mut app, 'a');
        assert_eq!(app.operation_name(), None, "{:?}", app.message);
    }
}
