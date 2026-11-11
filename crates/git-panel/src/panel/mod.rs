use crate::keys;
use git_core::{Kind, Repo, Side};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::path::{Path, PathBuf};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;
use ui::{Pane, Theme, layout};

/// What the host should do after an event.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    /// The user chose a file to open (absolute path).
    Open(PathBuf),
    /// Leave the panel (Esc, `q`).
    Close,
    /// The user asked for the command palette: the host lists `actions()` with
    /// `ui::keys::choose` and runs the pick with `run`.
    Palette,
    /// The user asked for the keys help: the host shows `ui::keys::Help`.
    Help,
    /// An action the panel does not own (the host registered it with `Panel::with_actions`).
    Command(&'static str),
    /// The repository changed (stage, unstage, discard, commit): hosts may want
    /// to refresh their own views.
    Changed,
}

/// One changed file: what of it is staged and what is not (`git status -s` columns).
#[derive(Debug, Clone)]
struct Change {
    rela: PathBuf,
    staged: Option<Kind>,
    unstaged: Option<Kind>,
    /// A submodule: a folder that is a repository of its own (its change is a moved commit or inner edits).
    submodule: bool,
}

impl Change {
    /// Something of it is not staged yet (a change, a new file, a conflict).
    fn open(&self) -> bool {
        self.unstaged.is_some()
    }

    /// The kind that colors its row: what is not staged, else what is.
    fn kind(&self) -> Kind {
        self.unstaged.or(self.staged).unwrap_or(Kind::Modified)
    }
}

enum Row {
    /// A directory (single-child chains are joined: `src/lib`).
    Folder {
        path: PathBuf,
        label: String,
        depth: usize,
        count: usize,
        folded: bool,
    },
    File {
        change: Change,
        depth: usize,
    },
}

/// The changed files as a directory tree.
#[derive(Default)]
struct Node {
    dirs: std::collections::BTreeMap<String, Node>,
    files: Vec<Change>,
}

impl Node {
    fn count(&self) -> usize {
        self.files.len() + self.dirs.values().map(Node::count).sum::<usize>()
    }
}

enum Mode {
    Normal,
    /// Commit message prompt.
    Commit(Input),
    /// Discard confirmation for this file.
    Discard(PathBuf, bool),
}

struct Preview {
    key: (PathBuf, bool, u16, u16, u64),
    lines: Vec<Line<'static>>,
    /// Where the hunks start (for the jump keys).
    hunks: Vec<usize>,
}

pub struct Panel {
    repo: Repo,
    root: PathBuf,
    rows: Vec<Row>,
    /// Indices into `rows` of the selectable (file) rows.
    items: Vec<usize>,
    /// Position in `items`.
    cursor: usize,
    offset: usize,
    page: usize,
    branch: String,
    mode: Mode,
    message: Option<(String, bool)>,
    preview: Option<Preview>,
    /// Bumped when the repository changes, so cached diffs are dropped.
    version: u64,
    pscroll: usize,
    pheight: usize,
    /// The changed files as a tree of directories (the default) or one flat list; the folded folders.
    tree: bool,
    folded: std::collections::HashSet<PathBuf>,
    /// Every changed file, also the ones folded away.
    all: Vec<Change>,
    /// Title of the list pane instead of `git: <branch>`.
    title: Option<String>,
    keymap: ui::keys::Keymap,
}

fn kind_char(k: Kind) -> char {
    match k {
        Kind::Added => 'A',
        Kind::Modified => 'M',
        Kind::Deleted => 'D',
        Kind::Renamed => 'R',
        Kind::Typechange => 'T',
        Kind::Untracked => '?',
        Kind::Conflict => 'U',
    }
}

impl Panel {
    /// The panel for the repository containing `path`; `None` if there is none.
    pub fn new(path: &Path) -> Option<Panel> {
        Panel::with_actions(path, "space", Vec::new(), &[])
    }

    /// The panel with a leader key and the host's own `actions` added to its keymap (they come
    /// back as `Action::Command(id)`).
    pub fn with_actions(
        path: &Path,
        leader: &str,
        actions: Vec<ui::keys::Action>,
        groups: &[ui::keys::Group],
    ) -> Option<Panel> {
        let repo = Repo::discover(path)?;
        let all: Vec<ui::keys::Action> = keys::ACTIONS.iter().cloned().chain(actions).collect();
        let all_groups: Vec<ui::keys::Group> = keys::GROUPS.iter().chain(groups).cloned().collect();
        let keymap = ui::keys::Keymap::new(leader, &all, &all_groups);
        let root = repo.root().to_path_buf();
        let mut p = Panel {
            repo,
            root,
            rows: Vec::new(),
            items: Vec::new(),
            cursor: 0,
            offset: 0,
            page: 10,
            branch: String::new(),
            mode: Mode::Normal,
            message: None,
            preview: None,
            version: 0,
            pscroll: 0,
            pheight: 10,
            tree: true,
            folded: Default::default(),
            all: Vec::new(),
            title: None,
            keymap,
        };
        p.refresh();
        Some(p)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reread the status, keeping the cursor on the same file where possible.
    pub fn refresh(&mut self) {
        let keep = self.row_key(self.cursor);
        self.branch = self.repo.branch().unwrap_or_else(|| "(detached)".into());
        let status = self.repo.status().unwrap_or_default();
        self.all = status
            .iter()
            .filter_map(|(path, st)| {
                let (staged, unstaged) = if st.unstaged == Some(Kind::Conflict) {
                    (None, Some(Kind::Conflict))
                } else {
                    (st.staged, st.unstaged)
                };
                (staged.is_some() || unstaged.is_some()).then(|| Change {
                    rela: path.to_path_buf(),
                    submodule: status.is_submodule(path),
                    staged,
                    unstaged,
                })
            })
            .collect();
        self.all.sort_by(|a, b| a.rela.cmp(&b.rela));
        self.rows.clear();
        self.items.clear();
        if self.tree {
            let mut root = Node::default();
            for change in &self.all {
                let mut node = &mut root;
                if let Some(dir) = change.rela.parent() {
                    for part in dir.components() {
                        node = node
                            .dirs
                            .entry(part.as_os_str().to_string_lossy().into_owned())
                            .or_default();
                    }
                }
                node.files.push(change.clone());
            }
            self.push_node(&root, Path::new(""), 0);
        } else {
            for change in self.all.clone() {
                self.items.push(self.rows.len());
                self.rows.push(Row::File { change, depth: 0 });
            }
        }
        // Stay on the same row.
        self.cursor = keep
            .and_then(|(p, folder)| {
                (0..self.items.len())
                    .find(|&n| self.row_key(n).is_some_and(|(q, f)| q == p && f == folder))
            })
            .unwrap_or(self.cursor)
            .min(self.items.len().saturating_sub(1));
        self.version += 1;
        self.pscroll = 0;
    }

    /// Add `node` (a directory) to the rows: its folders first, then its files.
    fn push_node(&mut self, node: &Node, prefix: &Path, depth: usize) {
        for (name, child) in &node.dirs {
            let (mut label, mut child, mut path) = (name.clone(), child, prefix.join(name));
            while child.files.is_empty() && child.dirs.len() == 1 {
                let (n, c) = child.dirs.iter().next().expect("one directory");
                label = format!("{label}/{n}");
                path = path.join(n);
                child = c;
            }
            let folded = self.folded.contains(&path);
            self.items.push(self.rows.len());
            self.rows.push(Row::Folder {
                path: path.clone(),
                label,
                depth,
                count: child.count(),
                folded,
            });
            if !folded {
                self.push_node(child, &path, depth + 1);
            }
        }
        for change in &node.files {
            self.items.push(self.rows.len());
            self.rows.push(Row::File { change: change.clone(), depth });
        }
    }

    /// Path and whether it is a folder, of the `n`th selectable row.
    fn row_key(&self, n: usize) -> Option<(PathBuf, bool)> {
        match self.rows.get(*self.items.get(n)?)? {
            Row::File { change, .. } => Some((change.rela.clone(), false)),
            Row::Folder { path, .. } => Some((path.clone(), true)),
        }
    }

    /// The highlighted folder of the tree view.
    fn current_folder(&self) -> Option<PathBuf> {
        match self.rows.get(*self.items.get(self.cursor)?)? {
            Row::Folder { path, .. } => Some(path.clone()),
            _ => None,
        }
    }

    /// The changes below the folder `path`.
    fn changes_under(&self, path: &Path) -> Vec<&Change> {
        self.all.iter().filter(|c| c.rela.starts_with(path)).collect()
    }

    /// The highlighted file.
    fn current(&self) -> Option<&Change> {
        match self.rows.get(*self.items.get(self.cursor)?)? {
            Row::File { change, .. } => Some(change),
            Row::Folder { .. } => None,
        }
    }

    fn say(&mut self, msg: impl Into<String>, error: bool) {
        self.message = Some((msg.into(), error));
    }

    fn move_by(&mut self, delta: isize) {
        let last = self.items.len().saturating_sub(1) as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, last.max(0)) as usize;
    }

    fn finish(&mut self, what: &str, result: Result<(), git_core::Error>) -> Action {
        match result {
            Ok(()) => {
                self.say(what, false);
                self.refresh();
                Action::Changed
            }
            Err(e) => {
                self.say(e.to_string(), true);
                Action::None
            }
        }
    }

    /// Whether the commit message is being typed (so keys like space are text, not commands).
    pub fn is_typing(&self) -> bool {
        matches!(self.mode, Mode::Commit(..))
    }

    pub fn handle_event(&mut self, ev: &Event) -> Action {
        let Event::Key(key @ KeyEvent { kind, .. }) = ev else { return Action::None };
        if *kind == KeyEventKind::Release {
            return Action::None;
        }
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Commit(input) => return self.commit_key(input, ev, key),
            Mode::Discard(rela, untracked) => {
                return match key.code {
                    KeyCode::Char('y' | 'Y' | 'd' | 'D') | KeyCode::Enter => {
                        let r = self.repo.discard(&rela, untracked);
                        self.finish(if untracked { "deleted" } else { "discarded" }, r)
                    }
                    _ => Action::None,
                };
            }
            Mode::Normal => {}
        }
        self.message = None;
        match self.keymap.feed(key) {
            ui::keys::Feed::Run(id) => self.run(id),
            ui::keys::Feed::NoBinding => {
                let seq = self.keymap.rejected().to_string();
                self.say(format!("{seq} is not bound"), true);
                Action::None
            }
            ui::keys::Feed::Pending | ui::keys::Feed::Cancelled | ui::keys::Feed::Unbound => {
                Action::None
            }
        }
    }

    /// The which-key menu while a leader sequence is under way.
    pub fn menu(&self) -> Option<ui::keys::Menu> {
        self.keymap.menu()
    }

    /// Every action with its keys, for the palette and the help.
    pub fn keys_rows(&self) -> Vec<ui::keys::Row> {
        self.keymap.rows()
    }

    /// Scroll the diff to the next or previous hunk.
    fn jump_hunk(&mut self, forward: bool) {
        let Some(p) = &self.preview else { return };
        let target = if forward {
            pv::Anchors::next(&p.hunks, self.pscroll)
        } else {
            pv::Anchors::prev(&p.hunks, self.pscroll)
        };
        if let Some(t) = target {
            self.pscroll = t;
        }
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
            "nav.bottom" => self.cursor = self.items.len().saturating_sub(1),
            "preview.down" => self.pscroll += (self.pheight / 2).max(1),
            "preview.up" => self.pscroll = self.pscroll.saturating_sub((self.pheight / 2).max(1)),
            "preview.next_hunk" => self.jump_hunk(true),
            "preview.prev_hunk" => self.jump_hunk(false),
            "item.open" => {
                if let Some(key) = self.current_folder() {
                    if !self.folded.remove(&key) {
                        self.folded.insert(key);
                    }
                    self.refresh();
                    return Action::None;
                }
                if let Some(c) = self.current() {
                    return Action::Open(self.root.join(&c.rela));
                }
            }
            "change.stage" => {
                // One key for both: what is not staged yet gets staged, a file that is all staged is unstaged.
                let targets: Vec<Change> = match self.current_folder() {
                    Some(path) => self.changes_under(&path).into_iter().cloned().collect(),
                    None => self.current().cloned().into_iter().collect(),
                };
                if targets.is_empty() {
                    return Action::None;
                }
                if targets.iter().any(Change::open) {
                    let paths: Vec<PathBuf> =
                        targets.iter().filter(|c| c.open()).map(|c| c.rela.clone()).collect();
                    let r = self.repo.stage(&paths);
                    return self.finish("staged", r);
                }
                let paths: Vec<PathBuf> = targets.iter().map(|c| c.rela.clone()).collect();
                let r = self.repo.unstage(&paths);
                return self.finish("unstaged", r);
            }
            "change.unstage" => {
                let targets: Vec<PathBuf> = match self.current_folder() {
                    Some(path) => self
                        .changes_under(&path)
                        .into_iter()
                        .filter(|c| c.staged.is_some())
                        .map(|c| c.rela.clone())
                        .collect(),
                    None => self
                        .current()
                        .filter(|c| c.staged.is_some())
                        .map(|c| c.rela.clone())
                        .into_iter()
                        .collect(),
                };
                if !targets.is_empty() {
                    let r = self.repo.unstage(&targets);
                    return self.finish("unstaged", r);
                }
            }
            "change.toggle_all" => {
                // Everything staged already (nothing left to stage): take it all back out.
                if !self.all.is_empty() && !self.all.iter().any(Change::open) {
                    let r = self.repo.unstage_all();
                    return self.finish("unstaged everything", r);
                }
                let r = self.repo.stage_all();
                return self.finish("staged everything", r);
            }
            "change.discard" => {
                if self.current_folder().is_some() {
                    self.say("discard works on one file at a time", true);
                    return Action::None;
                }
                // Discard: only work-tree changes, never the staged ones.
                if let Some(c) = self.current().cloned() {
                    match c.unstaged {
                        Some(Kind::Conflict) => self.say("resolve the conflict first", true),
                        Some(kind) => self.mode = Mode::Discard(c.rela, kind == Kind::Untracked),
                        None => self.say("nothing to discard here (unstage it first)", true),
                    }
                }
            }
            "commit.new" => {
                if self.all.iter().any(|c| c.staged.is_some()) {
                    self.mode = Mode::Commit(Input::default());
                } else {
                    self.say("nothing staged to commit (stage with s)", true);
                }
            }
            "commit.amend" => {
                if self.all.iter().any(|c| c.staged.is_some()) {
                    let r = self.repo.amend(None);
                    return self.finish("amended the last commit", r);
                }
                self.say("nothing staged to add to the last commit", true);
            }
            "view.tree" => {
                self.tree = !self.tree;
                self.refresh();
                self.say(if self.tree { "tree view" } else { "list view" }, false);
            }
            "view.refresh" => {
                self.refresh();
                self.say("refreshed", false);
            }
            "palette" => return Action::Palette,
            "help" => return Action::Help,
            "quit" => return Action::Close,
            _ => {}
        }
        Action::None
    }

    fn commit_key(&mut self, mut input: Input, ev: &Event, key: &KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::None,
            KeyCode::Enter => {
                let message = input.value().to_string();
                match self.repo.create_commit(&message) {
                    Ok(summary) => {
                        self.say(format!("committed {summary}"), false);
                        self.refresh();
                        Action::Changed
                    }
                    Err(e) => {
                        self.say(e.to_string(), true);
                        // Keep what was typed so the message is not lost.
                        self.mode = Mode::Commit(input);
                        Action::None
                    }
                }
            }
            _ => {
                input.handle_event(ev);
                self.mode = Mode::Commit(input);
                Action::None
            }
        }
    }

    /// The footer rows: the commit prompt, a discard question, or the last
    /// message. While the commit prompt is open an error (a refused commit) is
    /// shown above it, so it is not hidden by what you are typing.
    fn footer(&self, theme: &Theme) -> Vec<Line<'static>> {
        let c = &theme.colors;
        let message = self.message.as_ref().map(|(m, err)| {
            Line::from(Span::styled(
                m.clone(),
                Style::new().fg(if *err { c.error } else { c.muted }.into()),
            ))
        });
        match &self.mode {
            Mode::Commit(input) => {
                let prompt = Line::from(vec![
                    Span::styled("commit: ", Style::new().fg(c.accent.into())),
                    Span::styled(input.value().to_string(), Style::new().fg(c.fg.into())),
                ]);
                message.into_iter().chain(std::iter::once(prompt)).collect()
            }
            Mode::Discard(rela, untracked) => {
                let what = if *untracked { "Delete" } else { "Discard" };
                vec![Line::from(Span::styled(
                    format!("{what} {}? d again", rela.display()),
                    Style::new().fg(c.warning.into()),
                ))]
            }
            Mode::Normal => message.into_iter().collect(),
        }
    }
}

mod render;

#[cfg(test)]
mod tests;
