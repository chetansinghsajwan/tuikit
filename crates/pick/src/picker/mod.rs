use crate::candidate::Candidate;
use crate::grep;
use git_core::{Repo, Status};
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Injector, Matcher, Nucleo};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;
use ui::{Pane, Theme, layout};

mod render;
mod types;
use types::*;
pub use types::{Action, LiveSource, Mode, PickerOptions, RowStyle};

#[cfg(test)]
mod tests;

pub struct Picker {
    live: Option<LiveSource>,
    row_style: RowStyle,
    /// The input carries times (`--timestamps`): the rows show how long ago.
    timestamps: bool,
    opts: PickerOptions,
    nucleo: Nucleo<Candidate>,
    matcher: Matcher,
    input: Input,
    /// The toggles and file filters of a grep: `alt-c` case, `alt-w` whole word, `alt-r` regex, `alt-i` include, `alt-e` exclude.
    grep_opts: grep::Options,
    include: Input,
    exclude: Input,
    /// Which line takes the typing in grep mode.
    field: Field,
    last_query: String,
    /// Index into the matched list.
    cursor: u32,
    offset: u32,
    marked: BTreeSet<String>,
    git: Option<(Repo, Status)>,
    preview: Option<Preview>,
    /// Rows visible in the list at the last render, for paging.
    page: u32,
    /// Stops the running grep search.
    cancel: Arc<AtomicBool>,
    /// First visible preview row (commit previews scroll).
    pscroll: usize,
    /// Pane title override (`--title`).
    title: Option<String>,
    /// Icon before the title (git pickers).
    icon: Option<ui::icons::Icon>,
    /// External preview command for candidates that are plain text (`--preview-cmd`).
    preview_cmd: Option<crate::preview_cmd::PreviewCmd>,
    /// Keys that accept and report themselves (`--expect`).
    expect: Vec<(String, KeyEvent)>,
    /// Fixed lines under the prompt (`--header-lines`).
    header: Vec<String>,
    /// A rule between the prompt and what is under it (`--separator`).
    separator: bool,
    /// Preview rows visible at the last render, for paging.
    pheight: usize,
    /// Show path candidates as a directory hierarchy (see `set_tree`).
    tree_on: bool,
    /// The hierarchy of the current results; `None` when the list is flat.
    tree: Option<crate::tree::Tree>,
    tree_dirty: bool,
    /// Above this many results the list stays flat (a tree of everything is no help).
    tree_limit: u32,
}

impl Picker {
    /// `notify` is called from worker threads when results changed; the host
    /// should wake its event loop and call `tick`.
    pub fn new(mut opts: PickerOptions, notify: Arc<dyn Fn() + Send + Sync>) -> Self {
        if opts.mode == Mode::Grep {
            opts.paths = true;
        }
        let config = if opts.paths { Config::DEFAULT.match_paths() } else { Config::DEFAULT };
        let git = if opts.git && opts.paths {
            Repo::discover(&opts.root).and_then(|r| r.status().ok().map(|s| (r, s)))
        } else {
            None
        };
        let mut picker = Picker {
            live: None,
            nucleo: Nucleo::new(config, notify, None, 1),
            matcher: Matcher::new(Config::DEFAULT),
            input: Input::new(opts.query.clone()),
            grep_opts: grep::Options::default(),
            include: Input::default(),
            exclude: Input::default(),
            field: Field::Query,
            last_query: String::new(),
            cursor: 0,
            offset: 0,
            marked: BTreeSet::new(),
            git,
            preview: None,
            page: 10,
            cancel: Arc::new(AtomicBool::new(false)),
            pscroll: 0,
            title: None,
            icon: None,
            preview_cmd: None,
            row_style: RowStyle::default(),
            timestamps: false,
            expect: Vec::new(),
            header: Vec::new(),
            separator: true,
            tree_on: false,
            tree: None,
            tree_dirty: true,
            tree_limit: 20_000,
            pheight: 10,
            opts,
        };
        picker.reparse();
        picker
    }

    /// Take the candidates from `source` for the current query and every later one.
    pub fn set_live(&mut self, source: LiveSource) {
        self.live = Some(source);
        self.reparse();
    }

    /// Where candidates are pushed in `Mode::Fuzzy`. (Grep mode feeds itself.)
    pub fn injector(&self) -> Injector<Candidate> {
        self.nucleo.injector()
    }

    /// Highlight the rows as code, number them (`RowStyle`); rows that carry a time show its age.
    pub fn set_timestamps(&mut self, on: bool) {
        self.timestamps = on;
    }

    pub fn set_row_style(&mut self, style: RowStyle) {
        self.row_style = style;
    }

    /// Keys (`ctrl-e`, `alt-x`, `f2`, ...) that accept and report themselves
    /// through `Action::AcceptWith`. Fails on a name that is not understood.
    pub fn set_expect(&mut self, keys: &[String]) -> Result<(), String> {
        let mut out = Vec::new();
        for k in keys {
            let ev = crate::keys::parse(k).ok_or_else(|| format!("unknown key: {k}"))?;
            out.push((k.clone(), ev));
        }
        self.expect = out;
        Ok(())
    }

    /// Fixed lines shown under the prompt, e.g. a table's column names.
    pub fn set_separator(&mut self, on: bool) {
        self.separator = on;
    }

    pub fn set_header(&mut self, lines: Vec<String>) {
        self.header = lines;
    }

    /// Title of the list pane instead of `pick` / `grep`.
    pub fn set_icon(&mut self, icon: ui::icons::Icon) {
        self.icon = Some(icon);
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
    }

    /// Preview plain-text candidates with `cmd` (run through `sh -c`; `{}`, `{N}`
    /// and `{N..}` are replaced by the highlighted line or its fields, split on
    /// `delimiter` or whitespace). Turns the preview pane on.
    pub fn set_preview_command(&mut self, cmd: impl Into<String>, delimiter: Option<String>) {
        self.preview_cmd = Some(crate::preview_cmd::PreviewCmd { cmd: cmd.into(), delimiter });
        self.opts.preview = true;
    }

    /// Put the cursor on the `index`-th result (0-based), e.g. to preselect the
    /// previous buffer. Takes effect once the candidates are in (see `tick`).
    pub fn set_cursor(&mut self, index: u32) {
        // Let the matcher finish the candidates pushed so far (a few ms at most).
        for _ in 0..40 {
            if !self.nucleo.tick(5).running {
                break;
            }
        }
        self.cursor = index;
        self.clamp();
    }

    /// Advance the matcher. Returns true if the result list changed (redraw).
    pub fn tick(&mut self) -> bool {
        let changed = self.nucleo.tick(10).changed;
        if changed {
            self.clamp();
            self.tree_dirty = true;
        }
        changed
    }

    /// Show path candidates (files, grep results) as a directory hierarchy:
    /// folders as header rows, items indented beneath, folders holding a single
    /// folder merged. Up/Down walk the items in drawing order; the cursor still
    /// starts on the best match. Candidates that are not plain file paths
    /// (buffers, git objects, folders) and result sets above a limit keep the
    /// flat list.
    pub fn set_tree(&mut self, on: bool) {
        self.tree_on = on;
        self.tree_dirty = true;
        self.offset = 0;
    }

    /// Where a candidate sits in the hierarchy: its folders and a sort key within
    /// the folder. `None` for candidates that have no place in a path tree.
    fn tree_entry(idx: u32, c: &Candidate) -> Option<crate::tree::Entry> {
        if c.buffer.is_some() || c.git.is_some() {
            return None;
        }
        let split = |p: &str| -> Vec<String> {
            p.split('/').filter(|s| !s.is_empty() && *s != ".").map(String::from).collect()
        };
        if let Some(loc) = &c.loc {
            // A grep match: the file is the folder, its matches the items.
            return Some(crate::tree::Entry { idx, dirs: split(&loc.path), sort: format!("{:010}", loc.line) });
        }
        if c.display.ends_with('/') {
            return None;
        }
        let mut parts = split(&c.display);
        let name = parts.pop()?;
        Some(crate::tree::Entry { idx, dirs: parts, sort: name.to_lowercase() })
    }

    /// Rebuild the hierarchy if the results changed since the last time.
    fn refresh_tree(&mut self) {
        if !self.tree_dirty {
            return;
        }
        self.tree_dirty = false;
        if !self.tree_on {
            self.tree = None;
            return;
        }
        let n = self.matched();
        if n == 0 || n > self.tree_limit {
            self.tree = None;
            return;
        }
        let snap = self.nucleo.snapshot();
        let mut entries = Vec::with_capacity(n as usize);
        for idx in 0..n {
            let Some(item) = snap.get_matched_item(idx) else { continue };
            match Self::tree_entry(idx, item.data) {
                Some(e) => entries.push(e),
                None => {
                    self.tree = None;
                    return;
                }
            }
        }
        let was_tree = self.tree.is_some();
        self.tree = Some(crate::tree::build(entries, !self.input.value().is_empty()));
        if !was_tree {
            self.offset = 0;
            // With nothing typed the list starts at its top: the cursor takes the first row
            // as drawn (folders first), not the first match in path order.
            if self.input.value().is_empty()
                && let Some(&first) = self.tree.as_ref().and_then(|t| t.order.first())
            {
                self.cursor = first;
            }
        }
    }

    fn reparse(&mut self) {
        let q = self.input.value().to_string();
        if let Some(live) = &self.live {
            // The source does the filtering: stop the old request, drop its results, ask again.
            self.cancel.store(true, Ordering::Relaxed);
            self.cancel = Arc::new(AtomicBool::new(false));
            self.nucleo.restart(true);
            live(&q, self.nucleo.injector(), self.cancel.clone());
            self.last_query = q;
            self.cursor = 0;
            self.offset = 0;
            self.tree_dirty = true;
            return;
        }
        match self.opts.mode {
            Mode::Fuzzy => {
                let append = q.starts_with(&self.last_query) && !self.last_query.is_empty();
                self.nucleo.pattern.reparse(0, &q, CaseMatching::Smart, Normalization::Smart, append);
            }
            Mode::Grep => {
                self.restart_grep();
            }
        }
        self.last_query = q;
        self.cursor = 0;
        self.offset = 0;
        self.tree_dirty = true;
    }

    /// Stop the running grep, drop its results and search again with the query and the filters as they are.
    fn restart_grep(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.nucleo.restart(true);
        self.grep_opts.include = self.include.value().to_string();
        self.grep_opts.exclude = self.exclude.value().to_string();
        grep::spawn(
            self.opts.root.clone(),
            self.input.value().to_string(),
            self.grep_opts.clone(),
            self.nucleo.injector(),
            self.cancel.clone(),
        );
        self.cursor = 0;
        self.offset = 0;
        self.tree_dirty = true;
    }

    /// Keys of the grep filters: the toggles, and typing in the include and exclude lines. `true` when handled.
    fn grep_key(&mut self, ev: &Event, code: KeyCode, modifiers: KeyModifiers) -> bool {
        if self.opts.mode != Mode::Grep {
            return false;
        }
        let alt = modifiers.contains(KeyModifiers::ALT);
        if alt {
            match code {
                KeyCode::Char('c') => self.grep_opts.case_sensitive = !self.grep_opts.case_sensitive,
                KeyCode::Char('w') => self.grep_opts.whole_word = !self.grep_opts.whole_word,
                KeyCode::Char('r') => self.grep_opts.regex = !self.grep_opts.regex,
                KeyCode::Char('i') => {
                    self.field = if self.field == Field::Include { Field::Query } else { Field::Include };
                    return true;
                }
                KeyCode::Char('e') => {
                    self.field = if self.field == Field::Exclude { Field::Query } else { Field::Exclude };
                    return true;
                }
                _ => return false,
            }
            self.restart_grep();
            return true;
        }
        if modifiers.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('c') | KeyCode::Char('g')) {
            return false;
        }
        let input = match self.field {
            Field::Query => return false,
            Field::Include => &mut self.include,
            Field::Exclude => &mut self.exclude,
        };
        match code {
            // Back to the query; the filter stays.
            KeyCode::Esc | KeyCode::Enter | KeyCode::Tab | KeyCode::Down | KeyCode::Up => self.field = Field::Query,
            _ => {
                if input.handle_event(ev).is_some() {
                    self.restart_grep();
                }
            }
        }
        true
    }

    fn matched(&self) -> u32 {
        self.nucleo.snapshot().matched_item_count()
    }

    fn clamp(&mut self) {
        let n = self.matched();
        if n == 0 {
            self.cursor = 0;
        } else if self.cursor >= n {
            self.cursor = n - 1;
        }
    }

    fn current(&self) -> Option<Candidate> {
        self.nucleo.snapshot().get_matched_item(self.cursor).map(|i| i.data.clone())
    }

    fn move_by(&mut self, delta: i64) {
        self.refresh_tree();
        // In the hierarchy, Up/Down walk the items in the order they are drawn.
        if let Some(tree) = &self.tree
            && let Some(&pos) = tree.pos_of.get(&self.cursor)
        {
            let last = tree.order.len() as i64 - 1;
            let to = (pos as i64 + delta).clamp(0, last) as usize;
            self.cursor = tree.order[to];
            return;
        }
        let n = i64::from(self.matched());
        if n == 0 {
            return;
        }
        self.cursor = (i64::from(self.cursor) + delta).clamp(0, n - 1) as u32;
    }

    pub fn handle_event(&mut self, ev: &Event) -> Action {
        let Event::Key(KeyEvent { code, modifiers, kind, .. }) = ev else {
            return Action::None;
        };
        if *kind == KeyEventKind::Release {
            return Action::None;
        }
        let ctrl = modifiers.contains(KeyModifiers::CONTROL);
        // `--expect` keys accept and report which key was used.
        if let Some((name, _)) =
            self.expect.iter().find(|(_, want)| crate::keys::matches(&KeyEvent::new(*code, *modifiers), want))
        {
            let name = name.clone();
            let picked: Vec<String> = if self.marked.is_empty() {
                self.current().map(|c| c.output()).into_iter().collect()
            } else {
                self.marked.iter().cloned().collect()
            };
            return Action::AcceptWith(name, picked);
        }
        if self.grep_key(ev, *code, *modifiers) {
            return Action::None;
        }
        match code {
            KeyCode::Esc => return Action::Cancel,
            KeyCode::Char('c') | KeyCode::Char('g') if ctrl => return Action::Cancel,
            KeyCode::Enter => {
                let picked: Vec<String> = if self.marked.is_empty() {
                    self.current().map(|c| c.output()).into_iter().collect()
                } else {
                    self.marked.iter().cloned().collect()
                };
                return Action::Accept(picked);
            }
            KeyCode::Up => self.move_by(-1),
            KeyCode::Down => self.move_by(1),
            KeyCode::Char('p') if ctrl => self.move_by(-1),
            KeyCode::Char('n') if ctrl => self.move_by(1),
            KeyCode::Char('k') if ctrl => self.move_by(-5),
            KeyCode::Char('j') if ctrl => self.move_by(5),
            KeyCode::Char('d') if ctrl => self.pscroll += (self.pheight / 2).max(1),
            KeyCode::Char('u') if ctrl => self.pscroll = self.pscroll.saturating_sub((self.pheight / 2).max(1)),
            // Tables (CSV, spreadsheets) show as text and back.
            KeyCode::Char('t') if ctrl => {
                pv::set_raw_tables(!pv::raw_tables());
                self.preview = None;
            }
            KeyCode::PageUp => self.move_by(-i64::from(self.page)),
            KeyCode::PageDown => self.move_by(i64::from(self.page)),
            KeyCode::Tab => {
                if let Some(c) = self.current().map(|c| c.output()) {
                    if !self.marked.remove(&c) {
                        self.marked.insert(c);
                    }
                    self.move_by(1);
                }
            }
            _ => {
                if self.input.handle_event(ev).is_some() {
                    self.reparse();
                }
            }
        }
        Action::None
    }

    fn git_state(&self, cand: &str) -> Option<(char, git_core::Kind)> {
        let (repo, status) = self.git.as_ref()?;
        let rela = repo.relative(&self.opts.root.join(cand))?;
        let fs = status.get(&rela)?;
        let k = fs.unstaged.or(fs.staged)?;
        let c = match (fs.unstaged, k) {
            (Some(git_core::Kind::Untracked), _) => '?',
            (Some(git_core::Kind::Conflict), _) => 'U',
            (Some(git_core::Kind::Deleted), _) | (None, git_core::Kind::Deleted) => 'D',
            (Some(_), _) => 'M',
            (None, git_core::Kind::Added) => 'A',
            (None, _) => '+',
        };
        Some((c, k))
    }

    /// Name color of a ref, commit or stash row.
    fn git_item_color(theme: &Theme, g: &crate::candidate::GitItem) -> Color {
        use crate::candidate::GitKind::*;
        let c = &theme.colors;
        match g.kind {
            Branch => c.accent,
            Remote => c.info,
            Tag | Commit => c.warning,
            Stash => c.git.conflict,
        }
        .into()
    }

    fn git_color(theme: &Theme, kind: git_core::Kind) -> Color {
        use git_core::Kind::*;
        let g = &theme.colors.git;
        match kind {
            Added => g.added,
            Modified | Typechange => g.modified,
            Deleted => g.deleted,
            Renamed => g.renamed,
            Untracked => g.untracked,
            Conflict => g.conflict,
        }
        .into()
    }
}
