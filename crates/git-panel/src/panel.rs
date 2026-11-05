use git_core::{Kind, Repo, Side};
use crate::keys;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::path::{Path, PathBuf};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
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
    Folder { path: PathBuf, label: String, depth: usize, count: usize, folded: bool },
    File { change: Change, depth: usize },
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
    pub fn with_actions(path: &Path, leader: &str, actions: Vec<ui::keys::Action>, groups: &[ui::keys::Group]) -> Option<Panel> {
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
                let (staged, unstaged) = if st.unstaged == Some(Kind::Conflict) { (None, Some(Kind::Conflict)) } else { (st.staged, st.unstaged) };
                (staged.is_some() || unstaged.is_some()).then(|| Change { rela: path.to_path_buf(), staged, unstaged })
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
                        node = node.dirs.entry(part.as_os_str().to_string_lossy().into_owned()).or_default();
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
            .and_then(|(p, folder)| (0..self.items.len()).find(|&n| self.row_key(n).is_some_and(|(q, f)| q == p && f == folder)))
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
            self.rows.push(Row::Folder { path: path.clone(), label, depth, count: child.count(), folded });
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
            ui::keys::Feed::Pending | ui::keys::Feed::Cancelled | ui::keys::Feed::Unbound => Action::None,
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
        let target = if forward { pv::Anchors::next(&p.hunks, self.pscroll) } else { pv::Anchors::prev(&p.hunks, self.pscroll) };
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
                    let paths: Vec<PathBuf> = targets.iter().filter(|c| c.open()).map(|c| c.rela.clone()).collect();
                    let r = self.repo.stage(&paths);
                    return self.finish("staged", r);
                }
                let paths: Vec<PathBuf> = targets.iter().map(|c| c.rela.clone()).collect();
                let r = self.repo.unstage(&paths);
                return self.finish("unstaged", r);
            }
            "change.unstage" => {
                let targets: Vec<PathBuf> = match self.current_folder() {
                    Some(path) => self.changes_under(&path).into_iter().filter(|c| c.staged.is_some()).map(|c| c.rela.clone()).collect(),
                    None => self.current().filter(|c| c.staged.is_some()).map(|c| c.rela.clone()).into_iter().collect(),
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
        let message = self.message.as_ref().map(|(m, err)| Line::from(Span::styled(m.clone(), Style::new().fg(if *err { c.error } else { c.muted }.into()))));
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
                vec![Line::from(Span::styled(format!("{what} {}? d again", rela.display()), Style::new().fg(c.warning.into())))]
            }
            Mode::Normal => message.into_iter().collect(),
        }
    }

    /// Only the file list (no diff pane), for a narrow dock; `focused` colors
    /// the border like any focused pane.
    pub fn render_compact(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme, focused: bool) {
        self.render_list(area, buf, theme, focused);
    }

    /// Only the diff of the highlighted file, for a host that lays the panes out itself.
    pub fn render_preview_pane(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        self.render_preview(area, buf, theme);
    }

    /// Replace the list pane's title (`git: <branch>` by default).
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
    }

    /// The checked-out branch, as of the last refresh.
    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let [list_area, preview_area] = layout::main_preview(theme, area);
        self.render_list(list_area, buf, theme, true);
        self.render_preview(preview_area, buf, theme);
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme, focused: bool) {
        let c = &theme.colors;
        let title = self.title.clone().unwrap_or_else(|| format!("git: {}", self.branch));
        let pane = Pane::new(theme).title(&title).icon(ui::icons::GIT).focused(focused);
        let mut inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height == 0 || inner.width < 6 {
            return;
        }
        let footer = self.footer(theme);
        if !footer.is_empty() {
            let rows = (footer.len() as u16).min(inner.height);
            let top = inner.bottom() - rows;
            for (i, line) in footer.into_iter().rev().take(usize::from(rows)).collect::<Vec<_>>().into_iter().rev().enumerate() {
                Paragraph::new(line).render(Rect { y: top + i as u16, height: 1, ..inner }, buf);
            }
            if let Mode::Commit(input) = &self.mode {
                // The prompt is the last footer row.
                let (x, y) = (inner.x + 8 + input.visual_cursor() as u16, inner.bottom() - 1);
                if x < inner.right() {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_style(Style::new().add_modifier(Modifier::REVERSED));
                    }
                }
            }
            inner.height -= rows;
            if inner.height == 0 {
                return;
            }
        }
        if self.rows.is_empty() {
            buf.set_string(inner.x + 1, inner.y, "clean: nothing to commit", Style::new().fg(c.muted.into()));
            return;
        }

        self.page = usize::from(inner.height).max(1);
        let crow = self.items.get(self.cursor).copied().unwrap_or(0);
        if crow < self.offset {
            self.offset = crow;
        } else if crow >= self.offset + self.page {
            self.offset = crow + 1 - self.page;
        }
        let depths: Vec<usize> = self.rows.iter().map(|r| match r { Row::Folder { depth, .. } | Row::File { depth, .. } => *depth }).collect();
        let scope = ui::tree::active_scope(&depths, crow);
        let geo = ui::list::Rows::new(area, inner);
        for (i, row) in self.rows.iter().enumerate().skip(self.offset).take(self.page) {
            let y = inner.y + (i - self.offset) as u16;
            let selected = i == crow;
            match row {
                Row::Folder { label, depth, count, folded, .. } => {
                    ui::tree::file_row(theme, buf, &geo, y, &ui::tree::FileRow { depth: *depth, name: label, is_dir: true, open: !*folded, count: Some(*count), selected, focused: true, active_guide: scope.and_then(|s| s.col_at(i)), ..Default::default() });
                }
                Row::File { change, depth } => {
                    let name = if self.tree { change.rela.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() } else { change.rela.display().to_string() };
                    // Two marks like `git status -s`: what is staged (always in the staged color, green, so a staged file
                    // is told apart at a glance), then what is not (in the color of its change).
                    let staged = |kind: Option<Kind>| kind.map_or((' ', c.fg.into()), |k| (kind_char(k), c.success.into()));
                    let unstaged = |kind: Option<Kind>| kind.map_or((' ', c.fg.into()), |k| (kind_char(k), pv::kind_color(theme, k)));
                    ui::tree::file_row(
                        theme,
                        buf,
                        &geo,
                        y,
                        &ui::tree::FileRow {
                            depth: *depth,
                            name: &name,
                            tint: Some(pv::kind_color(theme, change.kind())),
                            selected,
                            focused: true,
                            signs: vec![staged(change.staged), unstaged(change.unstaged)],
                            active_guide: scope.and_then(|s| s.col_at(i)),
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }

    fn render_preview(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let current = self.current().map(|c| (c.unstaged.is_none(), c.rela.clone()));
        let title = current.as_ref().map(|(_, p)| p.display().to_string()).unwrap_or_default();
        let name = title.trim_end_matches('/').rsplit('/').next().unwrap_or(&title).to_string();
        let pane = Pane::new(theme).title(&title).icon(ui::icons::file(&name, false));
        let inner = pane.inner(area);
        pane.render(area, buf);
        let Some((staged, rela)) = current else {
            self.preview = None;
            return;
        };
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let key = (rela.clone(), staged, inner.width, inner.height, self.version);
        if self.preview.as_ref().map(|p| &p.key) != Some(&key) {
            let opts = pv::Options { width: Some(inner.width), height: None, git: false, focus: None };
            let side = if staged { Side::Staged } else { Side::Unstaged };
            let (lines, hunks) = match pv::preview_diff_marked(&self.root, &rela, side, theme, &opts) {
                Ok((l, a)) => (l, a.hunks),
                Err(e) => (vec![Line::from(Span::styled(e.to_string(), Style::new().fg(theme.colors.muted.into())))], Vec::new()),
            };
            self.preview = Some(Preview { key, lines, hunks });
            self.pscroll = 0;
        }
        self.pheight = usize::from(inner.height);
        if let Some(p) = &self.preview {
            self.pscroll = self.pscroll.min(p.lines.len().saturating_sub(1));
            let shown: Vec<Line<'static>> = p.lines.iter().skip(self.pscroll).take(usize::from(inner.height)).cloned().collect();
            Paragraph::new(shown).render(inner, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;
    use std::fs;
    use std::process::Command;

    /// Commits made through `Repo::create_commit` use the ambient git identity,
    /// which a clean build sandbox does not have.
    fn identity() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            for (k, v) in [("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t")] {
                // SAFETY: set once, before any test spawns git, to constants.
                unsafe { std::env::set_var(k, v) };
            }
        });
    }

    fn git(dir: &Path, args: &[&str]) {
        assert!(Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .status()
            .unwrap()
            .success());
    }

    /// A repo with: a.txt modified (unstaged), b.txt modified and staged, new.txt untracked.
    fn repo(name: &str) -> PathBuf {
        identity();
        let d = std::env::temp_dir().join(format!("git-panel-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        git(&d, &["init", "-q", "-b", "main"]);
        fs::write(d.join("a.txt"), "a1\na2\n").unwrap();
        fs::write(d.join("b.txt"), "b1\nb2\n").unwrap();
        git(&d, &["add", "."]);
        git(&d, &["commit", "-q", "-m", "init"]);
        fs::write(d.join("a.txt"), "a1\nA2 changed\n").unwrap();
        fs::write(d.join("b.txt"), "b1\nB2 staged\n").unwrap();
        git(&d, &["add", "b.txt"]);
        fs::write(d.join("new.txt"), "brand new\n").unwrap();
        d.canonicalize().unwrap()
    }

    fn key(c: KeyCode) -> Event {
        Event::Key(KeyEvent::new(c, KeyModifiers::NONE))
    }

    fn press(p: &mut Panel, s: &str) -> Action {
        let mut last = Action::None;
        for c in s.chars() {
            last = p.handle_event(&key(KeyCode::Char(c)));
        }
        last
    }

    fn draw(p: &mut Panel) -> Vec<String> {
        let area = Rect::new(0, 0, 100, 16);
        let mut b = Buffer::empty(area);
        p.render(area, &mut b, &Theme::default());
        (0..16).map(|y| (0..100).map(|x| b[(x, y)].symbol().to_string()).collect::<String>()).collect()
    }

    fn on(p: &mut Panel, name: &str) {
        let i = p.items.iter().position(|&i| matches!(&p.rows[i], Row::File { change, .. } if change.rela == Path::new(name))).unwrap_or_else(|| panic!("{name} listed"));
        p.cursor = i;
    }

    #[test]
    fn one_tree_with_two_marks_per_file_and_icons() {
        let mut p = Panel::new(&repo("sections")).unwrap();
        let text = draw(&mut p).join("\n");
        assert!(!text.contains("Staged (") && !text.contains("Unstaged (") && !text.contains("Untracked ("), "no sections: {text}");
        let pos = |s: &str| text.find(s).unwrap_or_else(|| panic!("{s} in\n{text}"));
        // Sorted by name; the marks sit at the right edge: the first is staged, the second not staged (`?` for a new file).
        assert!(pos("main") < pos("a.txt") && pos("a.txt") < pos("b.txt") && pos("b.txt") < pos("new.txt"), "{text}");
        assert!(text.contains(" M ") && text.contains("M  ") && text.contains(" ? "), "{text}");
        // A file that is partly staged is one row with both marks (`M M`).
        fs::write(p.root.join("b.txt"), "b1\nmore\n").unwrap();
        p.refresh();
        assert_eq!(p.all.iter().filter(|c| c.rela == Path::new("b.txt")).count(), 1);
        assert!(draw(&mut p).join("\n").contains("M M"));
    }

    #[test]
    fn staged_marks_are_drawn_in_the_staged_color() {
        let mut p = Panel::new(&repo("stagedcolor")).unwrap();
        let theme = Theme::default();
        let area = Rect::new(0, 0, 100, 16);
        let mut b = Buffer::empty(area);
        p.render(area, &mut b, &theme);
        let success: ratatui::style::Color = theme.colors.success.into();
        let row = |name: &str| (0..16).find(|&y| (0..100).map(|x| b[(x, y)].symbol().to_string()).collect::<String>().contains(name)).unwrap_or_else(|| panic!("{name} listed"));
        // b.txt is staged, a.txt is changed but not staged: the mark of the staged one is green, the other is not.
        let green_mark = |y: u16| (0..100).any(|x| b[(x, y)].symbol() == "M" && b[(x, y)].style().fg == Some(success));
        assert!(green_mark(row("b.txt")), "staged mark");
        assert!(!green_mark(row("a.txt")), "unstaged mark");
    }

    #[test]
    fn s_toggles_stage_and_unstage_and_the_cursor_stays_on_the_file() {
        let d = repo("toggle");
        let mut p = Panel::new(&d).unwrap();
        on(&mut p, "new.txt");
        assert_eq!(press(&mut p, "s"), Action::Changed);
        assert_eq!(p.current().map(|c| (c.rela.clone(), c.staged)), Some((PathBuf::from("new.txt"), Some(Kind::Added))));
        assert_eq!(press(&mut p, "s"), Action::Changed); // all staged: back out
        assert_eq!(p.current().map(|c| c.unstaged), Some(Some(Kind::Untracked)));
        // `u` only unstages.
        on(&mut p, "a.txt");
        assert_eq!(press(&mut p, "u"), Action::None);
        // A partly staged file: s stages the rest, s again unstages it.
        on(&mut p, "b.txt");
        fs::write(d.join("b.txt"), "b1\nmore\n").unwrap();
        p.refresh();
        assert_eq!(press(&mut p, "s"), Action::Changed);
        assert!(p.current().is_some_and(|c| c.unstaged.is_none() && c.staged.is_some()));
        assert_eq!(press(&mut p, "s"), Action::Changed);
        assert!(p.current().is_some_and(|c| c.staged.is_none()), "{:?}", p.current());
        // `a` toggles: it stages everything while something is left, and unstages it all once all is staged.
        assert_eq!(press(&mut p, "a"), Action::Changed);
        assert!(p.all.iter().all(|c| !c.open()));
        assert_eq!(press(&mut p, "a"), Action::Changed);
        assert!(p.all.iter().all(|c| c.staged.is_none()));
    }

    #[test]
    fn the_tree_groups_by_folder_stages_a_folder_folds_and_toggles_to_a_list() {
        let d = repo("tree");
        fs::create_dir_all(d.join("src/deep/er")).unwrap();
        fs::write(d.join("src/one.rs"), "1\n").unwrap();
        fs::write(d.join("src/deep/er/two.rs"), "2\n").unwrap();
        let mut p = Panel::new(&d).unwrap();
        let text = draw(&mut p).join("\n");
        assert!(text.contains("src 2"), "{text}");
        assert!(text.contains("deep/er 1"), "a chain of single folders is one row: {text}");
        assert!(!text.contains("src/one.rs"), "files show their name only in the tree");
        // Enter on the folder folds it.
        let at = p.items.iter().position(|&i| matches!(&p.rows[i], Row::Folder { path, .. } if path == Path::new("src"))).unwrap();
        p.cursor = at;
        p.handle_event(&key(KeyCode::Enter));
        let text = draw(&mut p).join("\n");
        assert!(text.contains("src 2") && !text.contains("one.rs"), "{text}");
        // s on the folder stages everything below it, s again unstages it.
        assert_eq!(press(&mut p, "s"), Action::Changed);
        assert!(p.all.iter().filter(|c| c.rela.starts_with("src")).all(|c| !c.open()), "{:?}", p.message);
        assert_eq!(press(&mut p, "s"), Action::Changed);
        assert!(p.all.iter().filter(|c| c.rela.starts_with("src")).all(|c| c.staged.is_none()));
        // t switches to the flat list with the full paths.
        press(&mut p, "t");
        let text = draw(&mut p).join("\n");
        assert!(text.contains("src/one.rs"), "{text}");
    }

    #[test]
    fn x_asks_before_discarding_and_never_touches_staged_changes() {
        let d = repo("discard");
        let mut p = Panel::new(&d).unwrap();
        on(&mut p, "b.txt"); // staged
        press(&mut p, "d");
        assert!(matches!(p.mode, Mode::Normal), "staged files are not discarded");
        assert!(p.message.as_ref().is_some_and(|(_, err)| *err));

        on(&mut p, "a.txt");
        press(&mut p, "d");
        assert!(draw(&mut p).join("\n").contains("Discard a.txt? d again"));
        assert_eq!(press(&mut p, "n"), Action::None);
        assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "a1\nA2 changed\n");
        press(&mut p, "d");
        assert_eq!(press(&mut p, "d"), Action::Changed);
        assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "a1\na2\n");

        on(&mut p, "new.txt");
        press(&mut p, "d");
        assert!(draw(&mut p).join("\n").contains("Delete new.txt? d again"));
        assert_eq!(press(&mut p, "y"), Action::Changed);
        assert!(!d.join("new.txt").exists());
    }

    #[test]
    fn c_commits_the_staged_files_with_the_typed_message() {
        let d = repo("commit");
        let mut p = Panel::new(&d).unwrap();
        // Empty message: refused, the prompt stays so nothing typed is lost.
        press(&mut p, "c");
        assert!(draw(&mut p).join("\n").contains("commit: "));
        assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::None);
        assert!(matches!(p.mode, Mode::Commit(_)) && p.message.as_ref().is_some_and(|(_, e)| *e));
        let shown = draw(&mut p).join("\n");
        assert!(shown.contains("empty commit message") && shown.contains("commit: "), "the refusal is visible above the prompt:\n{shown}");
        press(&mut p, "fix b");
        assert!(draw(&mut p).join("\n").contains("commit: fix b"));
        assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::Changed);
        let log = p.repo.commits(2).unwrap();
        assert_eq!(log[0].subject, "fix b");
        // What was staged is gone from the list; the other files remain.
        let text = draw(&mut p).join("\n");
        assert!(!text.contains("b.txt") && text.contains("a.txt") && text.contains("committed "), "{text}");
        // Esc cancels the prompt.
        press(&mut p, "c"); // nothing staged now
        assert!(p.message.as_ref().is_some_and(|(m, e)| *e && m.contains("nothing staged")));
    }

    #[test]
    fn enter_opens_the_file_and_the_preview_shows_the_right_side_of_the_diff() {
        let d = repo("preview");
        let mut p = Panel::new(&d).unwrap();
        on(&mut p, "a.txt");
        let text = draw(&mut p).join("\n");
        assert!(text.contains("A2 changed") && !text.contains("B2 staged"), "unstaged diff of a.txt:\n{text}");
        on(&mut p, "b.txt");
        let text = draw(&mut p).join("\n");
        assert!(text.contains("B2 staged"), "staged diff of b.txt:\n{text}");
        assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::Open(d.join("b.txt")));
        assert_eq!(p.handle_event(&key(KeyCode::Esc)), Action::Close);
    }

    #[test]
    fn a_clean_repo_and_a_non_repo() {
        let d = std::env::temp_dir().join(format!("git-panel-test-{}-clean", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        assert!(Panel::new(&d).is_none());
        git(&d, &["init", "-q", "-b", "main"]);
        let mut p = Panel::new(&d).unwrap();
        assert!(draw(&mut p).join("\n").contains("clean: nothing to commit"));
        assert_eq!(press(&mut p, "j"), Action::None); // navigation on an empty list is harmless
    }

    #[test]
    fn space_opens_the_leader_menu_with_the_actions_and_the_palette_and_help_ask_the_host() {
        let d = repo("leader");
        let mut p = Panel::new(&d).unwrap();
        assert_eq!(press(&mut p, " "), Action::None);
        let menu = p.menu().expect("a leader menu");
        let names: Vec<&str> = menu.entries.iter().map(|e| e.name.as_str()).collect();
        for want in ["stage or unstage", "unstage", "commit", "discard", "commands"] {
            assert!(names.contains(&want), "{want} in {names:?}");
        }
        // Space then Space is the palette; `?` is the help; the host's own actions come back as commands.
        assert_eq!(press(&mut p, " "), Action::Palette);
        assert_eq!(press(&mut p, "?"), Action::Help);
        let extra = vec![ui::keys::Action { id: "pane.next", name: "next pane", desc: "x", basic: true, keys: &["tab"] }];
        let mut p = Panel::with_actions(&d, "space", extra, &[]).unwrap();
        assert_eq!(p.handle_event(&Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))), Action::Command("pane.next"));
        // The rows for a palette: the basic keys are marked.
        assert!(p.keys_rows().iter().any(|r| r.id == "nav.down" && r.basic) && p.keys_rows().iter().any(|r| r.id == "commit.new" && !r.basic));
    }
}
