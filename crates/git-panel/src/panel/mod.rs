//! The git panel's state and behaviour: the change list, key handling and the git actions.
//! Drawing lives in `render`.

use crate::keys;
use git_core::{Kind, Repo, Side};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::path::{Path, PathBuf};
use tui_input::Input;
use ui::footer::Footer;
use ui::prompt::Typed;
use ui::{Component, Pane, layout};

/// What the host should do after an event.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
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
    /// An action the panel does not own (the host registered it with `Panel::with_commands`).
    Command(&'static str),
    /// The repository changed (stage, unstage, discard, commit): hosts may want
    /// to refresh their own views.
    Changed,
}

/// One changed file: what of it is staged and what is not (`git status -s` columns).
#[derive(Debug, Clone)]
struct Change {
    rel_path: PathBuf,
    staged: Option<Kind>,
    unstaged: Option<Kind>,
    /// A submodule: a folder that is a repository of its own (its change is a moved commit or inner
    /// edits).
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

/// One line of the change list: a folder or a changed file.
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
    /// How many changed files are in this folder, counting the sub-folders.
    fn count(&self) -> usize {
        self.files.len() + self.dirs.values().map(Node::count).sum::<usize>()
    }
}

/// What the keys do now: move around, type a commit message, or answer a discard question.
enum Mode {
    Normal,
    /// Commit message prompt.
    Commit(Input),
    /// Discard confirmation for this file.
    Discard(PathBuf, bool),
}

/// The cached diff of the highlighted file, so it is not recomputed on every frame.
struct Preview {
    key: (PathBuf, bool, u16, u16, u64),
    lines: Vec<Line<'static>>,
    /// Where the hunks start (for the jump keys).
    hunks: Vec<usize>,
}

/// The git panel: the repository, the change list rows, the cursor, the diff preview and the
/// keymap.
pub struct Panel {
    repo: Repo,
    root: PathBuf,
    rows: Vec<Row>,
    /// The cursor and the scroll over `rows`.
    list: ui::list::List,
    branch: String,
    mode: Mode,
    message: Option<ui::footer::Notice>,
    preview: Option<Preview>,
    /// Bumped when the repository changes, so cached diffs are dropped.
    version: u64,
    scroll: ui::scroll::PreviewScroll,
    /// The changed files as a tree of directories (the default) or one flat list; the folded
    /// folders.
    tree: bool,
    folded: std::collections::HashSet<PathBuf>,
    /// Every changed file, also the ones folded away.
    all: Vec<Change>,
    /// Title of the list pane instead of `git: <branch>`.
    title: Option<String>,
    keymap: ui::keys::Keymap,
}

/// The one-letter mark of a change kind, as in `git status -s`.
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
        Panel::with_commands(path, "space", ui::keys::Commands::default(), &[])
    }

    /// The panel with a leader key and the host's own `commands` added to its keymap (they come
    /// back as `Outcome::Command(id)`).
    pub fn with_commands(
        path: &Path,
        leader: &str,
        commands: ui::keys::Commands,
        groups: &[ui::keys::Group],
    ) -> Option<Panel> {
        let repo = Repo::discover(path)?;
        let mut all = keys::commands();
        all.extend(&commands);
        let all_groups: Vec<ui::keys::Group> = keys::GROUPS.iter().chain(groups).cloned().collect();
        let keymap = ui::keys::Keymap::new(leader, &all, &all_groups);
        let root = repo.root().to_path_buf();
        let mut p = Panel {
            repo,
            root,
            rows: Vec::new(),
            list: ui::list::List::new(),
            branch: String::new(),
            mode: Mode::Normal,
            message: None,
            preview: None,
            version: 0,
            scroll: Default::default(),
            tree: true,
            folded: Default::default(),
            all: Vec::new(),
            title: None,
            keymap,
        };
        p.refresh();
        Some(p)
    }

    /// The repository's root folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reread the status, keeping the cursor on the same file where possible.
    pub fn refresh(&mut self) {
        let keep = self.row_key(self.list.cursor());
        self.branch = self.repo.branch().unwrap_or_else(|| "(detached)".into());
        // Deep: files changed inside a submodule are rows too; the repo routes their actions.
        let status = self.repo.status_deep().unwrap_or_default();
        self.all = status
            .iter()
            .filter_map(|(path, st)| {
                let (staged, unstaged) = if st.unstaged == Some(Kind::Conflict) {
                    (None, Some(Kind::Conflict))
                } else {
                    (st.staged, st.unstaged)
                };
                (staged.is_some() || unstaged.is_some()).then(|| Change {
                    rel_path: path.to_path_buf(),
                    submodule: status.is_submodule(path),
                    staged,
                    unstaged,
                })
            })
            .collect();
        self.all.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        self.rows.clear();
        if self.tree {
            let mut root = Node::default();
            for change in &self.all {
                // A submodule with changes inside is the folder of those files, not a second row
                // (staging the folder stages its recorded commit too).
                if change.submodule
                    && self.all.iter().any(|c| {
                        c.rel_path != change.rel_path && c.rel_path.starts_with(&change.rel_path)
                    })
                {
                    continue;
                }
                let mut node = &mut root;
                if let Some(dir) = change.rel_path.parent() {
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
                self.rows.push(Row::File { change, depth: 0 });
            }
        }
        // Stay on the same row.
        let at = keep
            .and_then(|(p, folder)| {
                (0..self.rows.len())
                    .find(|&n| self.row_key(n).is_some_and(|(q, f)| q == p && f == folder))
            })
            .unwrap_or(self.list.cursor());
        self.list.set_len(self.rows.len());
        self.list.set_cursor(at);
        self.version += 1;
        self.scroll.reset();
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
            self.rows.push(Row::File { change: change.clone(), depth });
        }
    }

    /// Path and whether it is a folder, of row `n`.
    fn row_key(&self, n: usize) -> Option<(PathBuf, bool)> {
        match self.rows.get(n)? {
            Row::File { change, .. } => Some((change.rel_path.clone(), false)),
            Row::Folder { path, .. } => Some((path.clone(), true)),
        }
    }

    /// The highlighted folder of the tree view.
    fn current_folder(&self) -> Option<PathBuf> {
        match self.rows.get(self.list.cursor())? {
            Row::Folder { path, .. } => Some(path.clone()),
            _ => None,
        }
    }

    /// The changes below the folder `path`.
    fn changes_under(&self, path: &Path) -> Vec<&Change> {
        self.all.iter().filter(|c| c.rel_path.starts_with(path)).collect()
    }

    /// The highlighted file.
    fn current(&self) -> Option<&Change> {
        match self.rows.get(self.list.cursor())? {
            Row::File { change, .. } => Some(change),
            Row::Folder { .. } => None,
        }
    }

    /// Shows a message in the footer; `error` colours it as a failure.
    fn show_message(&mut self, msg: impl Into<String>, error: bool) {
        self.message = Some(ui::footer::Notice::new(msg, error));
    }

    /// Turns the result of a git change into a footer message and a refresh, and tells the host the
    /// repo changed.
    fn finish(&mut self, what: &str, result: Result<(), git_core::Error>) -> Outcome {
        match result {
            Ok(()) => {
                self.show_message(what, false);
                self.refresh();
                Outcome::Changed
            }
            Err(e) => {
                self.show_message(e.to_string(), true);
                Outcome::None
            }
        }
    }

    /// Whether the commit message is being typed (so keys like space are text, not commands).
    pub fn is_typing(&self) -> bool {
        matches!(self.mode, Mode::Commit(..))
    }

    /// Whether the commit prompt or the discard question is open, so the keys are the panel's own
    /// even though the host owns the keymap.
    pub fn captures_input(&self, _ev: &Event) -> bool {
        !matches!(self.mode, Mode::Normal)
    }

    /// Feeds an event the panel claimed (see `captures`) when the host owns the keymap, and returns
    /// what the host should do about it.
    pub fn handle_captured(&mut self, ev: &Event) -> Outcome {
        let Some(key) = ui::keys::pressed(ev) else { return Outcome::None };
        self.modal_key(ev, key).unwrap_or(Outcome::None)
    }

    /// Routes a key to the open commit prompt or discard question; `None` when neither is open.
    fn modal_key(&mut self, ev: &Event, key: &KeyEvent) -> Option<Outcome> {
        Some(match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Commit(input) => self.commit_key(input, ev),
            Mode::Discard(rel_path, untracked) if ui::prompt::confirms(key.code) => {
                let r = self.repo.discard(&rel_path, untracked);
                self.finish(if untracked { "deleted" } else { "discarded" }, r)
            }
            Mode::Discard(..) => Outcome::None,
            Mode::Normal => return None,
        })
    }

    /// Drops the footer message, as the next key press does.
    pub fn clear_message(&mut self) {
        self.message = None;
    }

    /// Shows a message in the footer; `error` colours it as a failure.
    pub fn notify(&mut self, msg: impl Into<String>, error: bool) {
        self.show_message(msg, error);
    }

    /// Handles a key press: first the commit and discard prompts, else the keymap.
    pub fn handle_event(&mut self, ev: &Event) -> Outcome {
        let Some(key) = ui::keys::pressed(ev) else { return Outcome::None };
        if let Some(outcome) = self.modal_key(ev, key) {
            return outcome;
        }
        self.message = None;
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

    /// Every action with its keys, for the palette and the help.
    pub fn keys_rows(&self) -> Vec<ui::keys::Row> {
        self.keymap.rows()
    }

    /// The scroll state of the diff, for a host that owns the keys and the wheel.
    pub fn preview_scroll(&mut self) -> &mut ui::scroll::PreviewScroll {
        &mut self.scroll
    }

    /// Scroll the diff to the next or previous hunk.
    fn jump_hunk(&mut self, forward: bool) {
        let Some(p) = &self.preview else { return };
        let target = if forward {
            pv::Anchors::next(&p.hunks, self.scroll.offset())
        } else {
            pv::Anchors::prev(&p.hunks, self.scroll.offset())
        };
        if let Some(t) = target {
            self.scroll.to(t);
        }
    }

    /// Run an action by id (what a key, the leader menu or a palette resolves to).
    pub fn run(&mut self, id: &'static str) -> Outcome {
        if !keys::is_own_action(id) {
            return Outcome::Command(id);
        }
        self.message = None;
        if ui::app::run_nav(&mut self.list, id) {
            return Outcome::None;
        }
        match id {
            "preview.down" | "preview.up" => {
                self.scroll.run(id);
            }
            "preview.next_hunk" => self.jump_hunk(true),
            "preview.prev_hunk" => self.jump_hunk(false),
            "item.open" => {
                if let Some(key) = self.current_folder() {
                    if !self.folded.remove(&key) {
                        self.folded.insert(key);
                    }
                    self.refresh();
                    return Outcome::None;
                }
                if let Some(c) = self.current() {
                    return Outcome::Open(self.root.join(&c.rel_path));
                }
            }
            "change.stage" => {
                // One key for both: what is not staged yet gets staged, a file that is all staged is unstaged.
                let targets: Vec<Change> = match self.current_folder() {
                    Some(path) => self.changes_under(&path).into_iter().cloned().collect(),
                    None => self.current().cloned().into_iter().collect(),
                };
                if targets.is_empty() {
                    return Outcome::None;
                }
                if targets.iter().any(Change::open) {
                    let paths: Vec<PathBuf> =
                        targets.iter().filter(|c| c.open()).map(|c| c.rel_path.clone()).collect();
                    let r = self.repo.stage(&paths);
                    return self.finish("staged", r);
                }
                let paths: Vec<PathBuf> = targets.iter().map(|c| c.rel_path.clone()).collect();
                let r = self.repo.unstage(&paths);
                return self.finish("unstaged", r);
            }
            "change.unstage" => {
                let targets: Vec<PathBuf> = match self.current_folder() {
                    Some(path) => self
                        .changes_under(&path)
                        .into_iter()
                        .filter(|c| c.staged.is_some())
                        .map(|c| c.rel_path.clone())
                        .collect(),
                    None => self
                        .current()
                        .filter(|c| c.staged.is_some())
                        .map(|c| c.rel_path.clone())
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
                    self.show_message("discard works on one file at a time", true);
                    return Outcome::None;
                }
                // Discard: only work-tree changes, never the staged ones.
                if let Some(c) = self.current().cloned() {
                    match c.unstaged {
                        Some(Kind::Conflict) => {
                            self.show_message("resolve the conflict first", true)
                        }
                        Some(kind) => {
                            self.mode = Mode::Discard(c.rel_path, kind == Kind::Untracked)
                        }
                        None => {
                            self.show_message("nothing to discard here (unstage it first)", true)
                        }
                    }
                }
            }
            "commit.new" => {
                if self.all.iter().any(|c| c.staged.is_some()) {
                    self.mode = Mode::Commit(Input::default());
                } else {
                    self.show_message("nothing staged to commit (stage with s)", true);
                }
            }
            "commit.amend" => {
                if self.all.iter().any(|c| c.staged.is_some()) {
                    let r = self.repo.amend(None);
                    return self.finish("amended the last commit", r);
                }
                self.show_message("nothing staged to add to the last commit", true);
            }
            "view.tree" => {
                self.tree = !self.tree;
                self.refresh();
                self.show_message(if self.tree { "tree view" } else { "list view" }, false);
            }
            "view.refresh" => {
                self.refresh();
                self.show_message("refreshed", false);
            }
            "palette" => return Outcome::Palette,
            "help" => return Outcome::Help,
            "quit" => return Outcome::Close,
            _ => {}
        }
        Outcome::None
    }

    /// Handles a key while the commit message is typed: Enter commits, Esc cancels, anything else
    /// edits.
    fn commit_key(&mut self, mut input: Input, ev: &Event) -> Outcome {
        match ui::prompt::typed(&mut input, ev) {
            Typed::Cancel => Outcome::None,
            Typed::Submit(message) => match self.repo.create_commit(&message) {
                Ok(summary) => {
                    self.show_message(format!("committed {summary}"), false);
                    self.refresh();
                    Outcome::Changed
                }
                Err(e) => {
                    self.show_message(e.to_string(), true);
                    // Keep what was typed so the message is not lost.
                    self.mode = Mode::Commit(input);
                    Outcome::None
                }
            },
            Typed::Editing => {
                self.mode = Mode::Commit(input);
                Outcome::None
            }
        }
    }

    /// The footer rows: the commit prompt, a discard question, or the last
    /// message. While the commit prompt is open an error (a refused commit) is
    /// shown above it, so it is not hidden by what you are typing.
    fn footer(&self, cx: &ui::Context) -> Vec<Line<'static>> {
        let message = self.message.as_ref().map(|n| Footer::Notice(n).line(cx));
        match &self.mode {
            Mode::Commit(input) => {
                let prompt = ui::prompt::line(cx, "commit: ", input.value());
                message.into_iter().chain(std::iter::once(prompt)).collect()
            }
            Mode::Discard(rel_path, untracked) => {
                let what = if *untracked { "Delete" } else { "Discard" };
                vec![Footer::Question(format!("{what} {}? d again", rel_path.display())).line(cx)]
            }
            Mode::Normal => message.into_iter().collect(),
        }
    }
}

/// Drawing: the change list and the diff pane.
mod render;

/// Tests for the panel.
#[cfg(test)]
mod tests;

/// The change rows are what the shared movement walks.
impl ui::app::Navigable for Panel {
    fn move_by(&mut self, delta: isize) {
        self.list.move_by(delta);
    }

    fn page_row_count(&self) -> usize {
        self.list.page_row_count()
    }

    fn move_to_top(&mut self) {
        self.list.move_to_top();
    }

    fn move_to_bottom(&mut self) {
        self.list.move_to_bottom();
    }
}
