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

/// How many commits the log pane loads.
const LOG_LIMIT: usize = 500;
/// How many commits the Commits tab loads.
const TAB_LOG_LIMIT: usize = 3000;
/// Most cells (two per lane) the graph column may take.
const GRAPH_MAX: usize = 24;

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

const ORDER: [Focus; 4] = [Focus::Files, Focus::Branches, Focus::Commits, Focus::Stashes];

enum PromptKind {
    /// Search the commits.
    Search,
    NewBranch,
    NewStash,
    /// Rename this local branch.
    RenameBranch(String),
    /// A branch at this commit.
    BranchAt(String),
    /// A tag at this commit.
    TagAt(String),
    /// A new message for the last commit.
    Reword,
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

/// The two tabs: the four panes, and the full commit graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Status,
    Commits,
}

enum Mode {
    Normal,
    Prompt(PromptKind, Input),
    Confirm(Confirm),
}

#[derive(Default, Clone, Copy)]
struct ListState {
    cursor: usize,
    offset: usize,
}

struct Preview {
    key: (Focus, String, u16),
    lines: Vec<Line<'static>>,
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
    /// The refs (branches, tags) pointing at each commit, by hash, for the labels.
    labels: std::collections::HashMap<String, Vec<RefInfo>>,
    /// The focused list pane fills the screen.
    zoom: bool,
    screen: Screen,
    /// What the graph and branch scope were before the Commits tab took them over.
    saved_view: (bool, bool),
    /// The Commits tab shows the diff of the commit under the cursor beside the list.
    detail: bool,
    /// The current commit search.
    search: Option<String>,
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
            pheight: 10,
            maps: [0, 1, 2].map(|i| Keymap::new("space", &crate::keys::pane_actions(i), crate::keys::GROUPS)),
            help: None,
            job: None,
            graph: true,
            all_branches: false,
            graph_rows: Vec::new(),
            graph_w: 0,
            labels: Default::default(),
            zoom: false,
            screen: Screen::Status,
            saved_view: (true, false),
            detail: false,
            search: None,
            watch,
        };
        app.reload_all();
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
        self.branches = refs;
        let limit = if self.screen == Screen::Commits { TAB_LOG_LIMIT } else { LOG_LIMIT };
        let commits = if self.all_branches { self.repo.commits_all(limit) } else { self.repo.commits(limit) };
        self.commits = commits.unwrap_or_default();
        self.graph_rows = if self.graph { git_core::graph(&self.commits) } else { Vec::new() };
        self.graph_w = self.graph_rows.iter().map(|r| r.cells.len()).max().unwrap_or(0).min(GRAPH_MAX);
        self.labels.clear();
        for r in &self.branches {
            self.labels.entry(r.oid.clone()).or_default().push(r.clone());
        }
        self.stashes = self.repo.stashes().unwrap_or_default();
        for (i, len) in [self.branches.len(), self.commits.len(), self.stashes.len()].into_iter().enumerate() {
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
            Focus::Branches => self.branches.len(),
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

    // ---- tabs and search --------------------------------------------------

    /// Switch tab. The Commits tab shows every branch with the graph and takes the Commits
    /// pane's list for that; leaving it restores what the pane showed.
    fn set_screen(&mut self, screen: Screen) {
        if screen == self.screen {
            return;
        }
        match screen {
            Screen::Commits => {
                self.saved_view = (self.all_branches, self.graph);
                self.all_branches = true;
                self.graph = true;
                self.zoom = false;
                self.focus = Focus::Commits;
            }
            Screen::Status => {
                (self.all_branches, self.graph) = self.saved_view;
                self.search = None;
            }
        }
        self.screen = screen;
        self.lists[1] = ListState::default();
        self.pscroll = 0;
        self.reload_history();
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
            Mode::Normal => {}
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
        // On the Commits tab the pane keys that leave the pane go back to the Status tab first.
        if self.screen == Screen::Commits && matches!(id, "focus.next" | "focus.prev" | "focus.files" | "focus.branches" | "focus.stashes" | "back") {
            if id == "back" && self.search.is_some() {
                self.search = None;
                return Quit::No;
            }
            self.set_screen(Screen::Status);
        }
        let mut moved = true;
        match id {
            "tab.next" | "tab.prev" => self.set_screen(if self.screen == Screen::Status { Screen::Commits } else { Screen::Status }),
            "search.start" => self.mode = Mode::Prompt(PromptKind::Search, Input::new(self.search.clone().unwrap_or_default())),
            "search.next" => self.search_step(1),
            "search.prev" => self.search_step(-1),
            "view.detail" => self.detail = !self.detail,
            "focus.next" => self.focus_by(1),
            "focus.prev" => self.focus_by(-1),
            "focus.files" => self.set_focus(Focus::Files),
            "focus.branches" => self.set_focus(Focus::Branches),
            "focus.commits" => self.set_focus(Focus::Commits),
            "focus.stashes" => self.set_focus(Focus::Stashes),
            "back" => self.set_focus(Focus::Files),
            "quit" => return Quit::Yes,
            "help" => self.help = Some(ui::keys::Help::default()),
            "palette" => return Quit::Palette,
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
            "view.all" => {
                self.all_branches = !self.all_branches;
                self.reload_history();
                self.say(if self.all_branches { "every branch" } else { "the checked-out branch" }, false);
            }
            "view.zoom" => self.zoom = !self.zoom,
            "preview.down" => self.pscroll += (self.pheight / 2).max(1),
            "preview.up" => self.pscroll = self.pscroll.saturating_sub((self.pheight / 2).max(1)),
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

    fn focus_by(&mut self, delta: isize) {
        let at = ORDER.iter().position(|f| *f == self.focus).unwrap_or(0) as isize;
        self.focus = ORDER[(at + delta).rem_euclid(ORDER.len() as isize) as usize];
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
                let Some(b) = self.branches.get(self.lists[0].cursor).cloned() else {
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
                        if b.kind == RefKind::Branch {
                            self.mode = Mode::Prompt(PromptKind::RenameBranch(b.name.clone()), Input::new(b.name));
                        } else {
                            self.say("only a local branch can be renamed", true);
                        }
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
                        if self.lists[1].cursor == 0 {
                            self.mode = Mode::Prompt(PromptKind::Reword, Input::new(k.subject));
                        } else {
                            self.say("only the last commit can be reworded here", true);
                        }
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

    fn prompt_key(&mut self, kind: PromptKind, mut input: Input, ev: &Event, key: &KeyEvent) {
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
                    PromptKind::BranchAt(oid) => {
                        let v = value.clone();
                        self.run(format!("created {value}"), move |r| r.create_branch_at(&v, &oid));
                    }
                    PromptKind::TagAt(oid) => {
                        let v = value.clone();
                        self.run(format!("tagged {value}"), move |r| r.create_tag(&v, &oid));
                    }
                    PromptKind::Reword => {
                        let v = value.clone();
                        self.run("reworded the last commit".to_string(), move |r| r.amend(Some(&v)));
                    }
                }
            }
            _ => {
                input.handle_event(ev);
                self.mode = Mode::Prompt(kind, input);
            }
        }
    }

    fn confirm_key(&mut self, c: Confirm, key: &KeyEvent) {
        if !matches!(key.code, KeyCode::Char('y' | 'Y') | KeyCode::Enter) {
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
        let labels = [ui::tabs::Label::new("Status"), ui::tabs::Label::new("Commits")];
        let active = usize::from(self.screen == Screen::Commits);
        ui::tabs::render(Rect { height: 1, ..area }, buf, &theme, &labels, active, false);
        let rest = Rect { y: area.y + 1, height: area.height.saturating_sub(1), ..area };
        match self.screen {
            Screen::Status => self.render_panes(rest, buf),
            Screen::Commits => self.render_commits_tab(rest, buf),
        }
    }

    /// The Commits tab: the whole graph of every branch full width, the diff of the commit beside it on request.
    fn render_commits_tab(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        if self.detail && body.width >= 60 {
            let [left, right] = ui::layout::main_preview(&theme, body);
            self.render_list(Focus::Commits, left, buf, &theme);
            self.render_preview(Focus::Commits, right, buf, &theme);
        } else {
            self.render_list(Focus::Commits, body, buf, &theme);
        }
        self.render_status(status, buf, &theme);
        self.render_overlays(area, buf, &theme);
    }

    /// The Status tab: the four panes and the preview.
    fn render_panes(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        let [left, right] = ui::layout::main_preview(&theme, body);
        // A zoomed list pane takes the whole body: no other panes, no preview.
        if self.zoom && Self::list_index(self.focus).is_some() {
            self.render_list(self.focus, body, buf, &theme);
            self.render_status(status, buf, &theme);
            self.render_overlays(area, buf, &theme);
            return;
        }

        // The focused pane takes the room; the others show a few rows.
        let constraints = ORDER.map(|f| if f == self.focus { Constraint::Min(7) } else { Constraint::Length(5) });
        let panes = Layout::vertical(constraints).split(left);
        self.files.render_compact(panes[0], buf, &theme, self.focus == Focus::Files);
        for (i, f) in [Focus::Branches, Focus::Commits, Focus::Stashes].into_iter().enumerate() {
            self.render_list(f, panes[i + 1], buf, &theme);
        }
        match self.focus {
            Focus::Files => self.files.render_preview_pane(right, buf, &theme),
            f => self.render_preview(f, right, buf, &theme),
        }
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
        if self.help.is_some() {
            let rows = self.keys_rows();
            if let Some(h) = self.help.as_mut() {
                h.render(area, buf, theme, "Keys", &rows);
            }
        }
    }

    fn render_list(&mut self, which: Focus, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let (title, n) = match which {
            Focus::Branches => ("Branches", self.branches.len()),
            Focus::Commits => ("Commits", self.commits.len()),
            _ => ("Stashes", self.stashes.len()),
        };
        let focused = self.focus == which;
        let mut pane = Pane::new(theme).title(title).focused(focused);
        if let Some(icon) = ui::icons::for_title(title) {
            pane = pane.icon(icon);
        }
        let inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height == 0 || inner.width < 4 {
            return;
        }
        let Some(li) = Self::list_index(which) else { return };
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

    /// The text of row `i`: the entry, and its age for the right edge.
    fn row(&self, which: Focus, i: usize, theme: &Theme) -> (Line<'static>, String) {
        let c = &theme.colors;
        let fg = Style::new().fg(c.fg.into());
        let muted = Style::new().fg(c.muted.into());
        match which {
            Focus::Branches => {
                let b = &self.branches[i];
                let name = if b.is_head {
                    Style::new().fg(c.success.into()).add_modifier(Modifier::BOLD)
                } else if b.kind == RefKind::Branch {
                    fg
                } else {
                    muted
                };
                (Line::from(Span::styled(b.name.clone(), name)), ago(b.time))
            }
            Focus::Commits => {
                let k = &self.commits[i];
                // `hash AB subject`: the author as two initials in a color of their own, like lazygit.
                let author = Span::styled(format!("{} ", initials(&k.author)), Style::new().fg(author_color(theme, &k.author)));
                let hit = self.commit_matches(i);
                let mut spans = Vec::new();
                if let Some(row) = self.graph_rows.get(i) {
                    spans.extend(self.graph_spans(row, theme));
                }
                // The author first, then the branches and tags at this commit as pills (the
                // checked-out branch is the solid one with the branch icon), then hash and subject.
                spans.push(author);
                for r in self.labels.get(&k.oid).into_iter().flatten() {
                    spans.extend(ref_pill(theme, r));
                }
                spans.push(Span::styled(format!("{} ", k.short), muted));
                spans.push(Span::styled(k.subject.clone(), if hit { Style::new().fg(c.warning.into()).add_modifier(Modifier::BOLD) } else { fg }));
                (Line::from(spans), ago(k.time))
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
            Focus::Branches => self.branches.get(self.lists[0].cursor).map(|b| b.name.clone()),
            Focus::Commits => self.commits.get(self.lists[1].cursor).map(|c| c.oid.clone()),
            Focus::Stashes => self.stashes.get(self.lists[2].cursor).map(|s| format!("stash@{{{}}}", s.index)),
            Focus::Files => None,
        }
    }

    fn render_preview(&mut self, which: Focus, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let rev = self.current_rev(which);
        let title = rev.clone().unwrap_or_default();
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
        if self.preview.as_ref().map(|p| &p.key) != Some(&key) {
            let opts = pv::Options { width: Some(inner.width), height: None, git: false, focus: None };
            let lines = match pv::preview_commit(&self.root, &rev, theme, &opts) {
                Ok(l) => l,
                Err(e) => vec![Line::from(Span::styled(e.to_string(), Style::new().fg(theme.colors.muted.into())))],
            };
            self.preview = Some(Preview { key, lines });
            self.pscroll = 0;
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
                    PromptKind::BranchAt(_) => "new branch here: ",
                    PromptKind::TagAt(_) => "tag name: ",
                    PromptKind::Reword => "message: ",
                };
                ui::prompt::cursor(buf, area, label, input.visual_cursor());
                ui::prompt::line(theme, label, input.value())
            }
            Mode::Confirm(cf) => {
                let q = match cf {
                    Confirm::DeleteBranch { name, force } => format!("{} branch {name}? (y/n)", if *force { "Force-delete" } else { "Delete" }),
                    Confirm::Merge(name) => format!("Merge {name} into {}? (y/n)", self.files.branch()),
                    Confirm::DropStash(i) => format!("Drop stash@{{{i}}}? (y/n)"),
                    Confirm::Rebase(name) => format!("Rebase {} onto {name}? (y/n)", self.files.branch()),
                    Confirm::Revert(_) => "Revert this commit? (y/n)".to_string(),
                    Confirm::Reset { .. } => "Reset hard to this commit and lose uncommitted changes? (y/n)".to_string(),
                    Confirm::ForcePush => format!("Force-push {} (with lease)? (y/n)", self.files.branch()),
                    Confirm::DeleteTag(name) => format!("Delete tag {name}? (y/n)"),
                    Confirm::DeleteRemote(name) => format!("Delete {name} on the remote? (y/n)"),
                };
                Line::from(Span::styled(q, Style::new().fg(c.warning.into())))
            }
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
            return "/ search  n N next match  p diff  enter checkout  C cherry-pick  space a/g all branches/graph  [ ] tab  ? keys";
        }
        match self.focus {
            Focus::Files => "s stage  u unstage  a/A all  x discard  c commit  enter edit  space space commands  ? keys",
            Focus::Branches => "enter checkout  n new  d delete  M merge  B rebase  space r fetch/pull/push  space space commands  ? keys",
            Focus::Commits => "enter checkout  C cherry-pick  R revert  w reword  b branch  t tag  space z reset  space space commands  ? keys",
            Focus::Stashes => "n new  a apply  p pop  d drop  space space commands  ? keys",
        }
    }
}

/// A branch or tag name as a pill with rounded ends. The checked-out branch is solid with the
/// branch icon; the others are a tint of their colour (branches blue, remotes grey, tags yellow).
fn ref_pill(theme: &Theme, r: &RefInfo) -> Vec<Span<'static>> {
    use ratatui::style::Color;
    let c = &theme.colors;
    let color = match (r.is_head, r.kind) {
        (true, _) => Color::from(c.success),
        (_, RefKind::Branch) => Color::from(c.info),
        (_, RefKind::Remote) => Color::from(c.muted),
        (_, RefKind::Tag) => Color::from(c.warning),
    };
    let (bg, fg, text) = if r.is_head {
        (color, Color::from(c.bg), format!("\u{e0a0} {}", r.name))
    } else {
        (ui::mix(color, Color::from(c.bg), 30), color, r.name.clone())
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
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(time);
    match (now - time).max(0) {
        0..60 => "now".into(),
        s @ 60..3600 => format!("{}m ago", s / 60),
        s @ 3600..86_400 => format!("{}h ago", s / 3600),
        s @ 86_400..2_592_000 => format!("{}d ago", s / 86_400),
        s @ 2_592_000..31_536_000 => format!("{}mo ago", s / 2_592_000),
        s => format!("{}y ago", s / 31_536_000),
    }
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
    fn shows_the_four_panes_and_the_log() {
        let d = repo("panes");
        let mut app = App::new(&d).unwrap();
        let all = screen(&mut app).join("\n");
        for title in ["Files", "Branches", "Commits", "Stashes"] {
            assert!(all.contains(title), "{title} in\n{all}");
        }
        assert!(all.contains("main") && all.contains("other"), "{all}");
    }

    #[test]
    fn focus_moves_with_tab_and_digits_and_q_quits() {
        let d = repo("focus");
        let mut app = App::new(&d).unwrap();
        press(&mut app, "3");
        assert_eq!(app.focus, Focus::Commits);
        app.handle_event(&key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Stashes);
        app.handle_event(&key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Files);
        app.handle_event(&Event::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)));
        assert_eq!(app.focus, Focus::Stashes);
        assert_eq!(app.handle_event(&key(KeyCode::Char('q'))), Quit::Yes);
    }

    #[test]
    fn branches_checkout_create_and_delete() {
        let d = repo("branches");
        let mut app = App::new(&d).unwrap();
        press(&mut app, "2");
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
        let other = app.branches.iter().position(|b| b.name == "other").unwrap();
        app.lists[0].cursor = other;
        press(&mut app, "d");
        assert!(matches!(app.mode, Mode::Confirm(_)));
        press(&mut app, "y");
        assert!(app.branches.iter().all(|b| b.name != "other"), "{:?}", app.branches);
        // The checked-out branch cannot be deleted.
        app.lists[0].cursor = app.branches.iter().position(|b| b.is_head).unwrap();
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
        press(&mut app, "2");
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
    fn commit_actions_check_out_tag_branch_reword_reset_and_revert() {
        let d = repo("commits");
        commit_file(&d, "b.txt", "two\n", "second");
        let mut app = App::new(&d).unwrap();
        press(&mut app, "3");
        // Reword the last commit.
        press(&mut app, "w");
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
        press(&mut app, "2");
        let at = |app: &App, name: &str| app.branches.iter().position(|b| b.name == name).unwrap();
        // Rebase feat onto main.
        app.lists[0].cursor = at(&app, "main");
        press(&mut app, "B");
        assert!(matches!(app.mode, Mode::Confirm(Confirm::Rebase(_))));
        press(&mut app, "y");
        assert!(d.join("m.txt").exists() && d.join("f.txt").exists(), "{:?}", app.message);
        // Rename it.
        app.lists[0].cursor = at(&app, "feat");
        press(&mut app, "R");
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
        press(&mut app, "2");
        let other = app.branches.iter().position(|b| b.name == "other").unwrap();
        app.lists[0].cursor = other;
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
