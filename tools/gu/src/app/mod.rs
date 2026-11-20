//! The `App` state of the screen; its behaviour is split over the submodules below.

use git_core::{
    CommitInfo, Operation, RefInfo, RefKind, Repo, ResetMode, StashInfo, SubmoduleInfo,
};
use git_panel::{Outcome as PanelOutcome, Panel};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::path::{Path, PathBuf};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;
use ui::app::{Host, Navigable, Request, Tabbed};
use ui::{Pane, Theme};

/// Loading history, status, refs and the commit list.
mod data;
/// Keys in each mode (normal, prompt, confirm, popup).
mod events;
/// Small drawing helpers: ref pills, author colors, widths.
mod pills;
/// The Refs tab's tree and the files of a commit.
mod refs;
/// Drawing the tabs and their panes.
mod render;
/// Tabs, focus and commit search.
mod search;
/// The kinds of things the screen holds: focus, modes, prompts, rows, filters.
mod types;
use pills::*;
pub use types::Outcome;
use types::*;

/// The whole git screen: the repository, what is loaded from it, and the UI state of every tab.
pub struct App {
    root: PathBuf,
    repo: Repo,
    theme: Theme,
    files: Panel,
    branches: Vec<RefInfo>,
    commits: Vec<CommitInfo>,
    stashes: Vec<StashInfo>,
    submodules: Vec<SubmoduleInfo>,
    /// The repos opened above this one (entering a submodule pushes, `leader U` pops).
    parents: Vec<PathBuf>,
    /// Cursor and scroll of the branches, commits, stashes and submodules panes.
    lists: [ListState; 4],
    focus: Focus,
    mode: Mode,
    message: Option<(String, bool)>,
    preview: Option<Preview>,
    preview_scroll: usize,
    /// The commit whose files were folded by hand (another commit starts with none).
    folded_commit: Option<String>,
    preview_height: usize,
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
    tab: Tab,
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

        let mut files = Panel::new(dir)?;
        files.set_title("Files");
        let mut app = App {
            root,
            repo,
            theme: Theme::load(),
            files,
            branches: Vec::new(),
            commits: Vec::new(),
            stashes: Vec::new(),
            submodules: Vec::new(),
            parents: Vec::new(),
            lists: [ListState::default(); 4],
            focus: Focus::Files,
            mode: Mode::Normal,
            message: None,
            preview: None,
            preview_scroll: 0,
            folded_commit: None,
            preview_height: 10,
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
            tab: Tab::Status,
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
        if let Some(i) = app
            .ref_rows
            .iter()
            .position(|r| matches!(r, RefRow::Item { index, .. } if app.branches[*index].is_head))
        {
            app.lists[0].cursor = i;
        }
        Some(app)
    }
}

/// Lets the shared app shell host the screen: it owns the keymaps, the help, the palette and the
/// terminal; the screen draws and runs its own actions.
impl ui::app::Screen for App {
    fn render(&mut self, area: Rect, buf: &mut Buffer, _theme: &Theme) {
        App::render(self, area, buf);
    }

    fn captures_input(&self, ev: &Event) -> bool {
        self.takes(ev)
    }

    fn on_event(&mut self, ev: &Event, host: &mut dyn Host) -> Request {
        let quit = self.typing(ev);
        self.finish(quit, host)
    }

    /// The Files panel's own actions run on the panel, every other one here.
    fn run_action(&mut self, id: &'static str, host: &mut dyn Host) -> Request {
        let quit = if self.focus == Focus::Files && git_panel::keys::is_own_action(id) {
            let action = self.files.run(id);
            self.files_action(action)
        } else {
            self.run_id(id)
        };
        self.finish(quit, host)
    }

    fn on_keypress(&mut self) {
        self.message = None;
        self.files.clear_message();
    }

    fn on_unbound_keys(&mut self, keys: &str) {
        let text = format!("{keys} is not bound");
        if self.focus == Focus::Files {
            self.files.notify(text, true);
        } else {
            self.show_message(text, true);
        }
    }

    fn on_tick(&mut self) -> bool {
        App::tick(self)
    }

    fn navigable(&mut self) -> Option<&mut dyn Navigable> {
        Some(self)
    }

    fn tabbed(&mut self) -> Option<&mut dyn Tabbed> {
        Some(self)
    }

    /// The focused pane: Files is context 0, the list panes follow.
    fn context(&self) -> usize {
        Self::list_index(self.focus).map_or(0, |i| i + 1)
    }
}

impl App {
    /// Turns what the screen decided into what the shell understands; an edit hands the terminal
    /// to the editor and reloads when it returns.
    fn finish(&mut self, quit: Outcome, host: &mut dyn Host) -> Request {
        match quit {
            Outcome::Continue => Request::None,
            Outcome::Quit => Request::Quit,
            Outcome::Palette => Request::Palette,
            Outcome::Help => Request::Help,
            Outcome::Edit(path) => {
                let ok = ui::app::with_suspended(host, || {
                    let editor = std::env::var("VISUAL")
                        .or_else(|_| std::env::var("EDITOR"))
                        .unwrap_or_else(|_| "vi".into());
                    std::process::Command::new("sh")
                        .arg("-c")
                        .arg(format!("{editor} {}", ui::popup::quote(&path.to_string_lossy())))
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false)
                })
                .unwrap_or(false);
                self.edited(ok);
                Request::None
            }
        }
    }
}

/// The Status, Commits and Refs tabs are fixed: they are visited, never opened or closed.
impl Tabbed for App {
    fn tab_next(&mut self) {
        self.select_tab(match self.tab {
            Tab::Status => Tab::Commits,
            Tab::Commits => Tab::Refs,
            Tab::Refs => Tab::Status,
        });
    }

    fn tab_prev(&mut self) {
        self.select_tab(match self.tab {
            Tab::Status => Tab::Refs,
            Tab::Commits => Tab::Status,
            Tab::Refs => Tab::Commits,
        });
    }

    fn tab_select(&mut self, index: usize) {
        match index {
            0 => self.select_tab(Tab::Status),
            1 => self.select_tab(Tab::Commits),
            2 => self.select_tab(Tab::Refs),
            _ => {}
        }
    }
}

/// The cursor of the focused pane is what the shared movement walks; the Files pane is the git
/// panel's own list.
impl Navigable for App {
    fn move_by(&mut self, delta: isize) {
        match Self::list_index(self.focus) {
            Some(li) => {
                let last = self.len(self.focus).saturating_sub(1) as isize;
                let st = &mut self.lists[li];
                st.cursor = (st.cursor as isize + delta).clamp(0, last) as usize;
                self.preview_scroll = 0;
            }
            None => self.files.move_by(delta),
        }
    }

    fn page_row_count(&self) -> usize {
        match Self::list_index(self.focus) {
            Some(_) => 10,
            None => self.files.page_row_count(),
        }
    }

    fn move_to_top(&mut self) {
        match Self::list_index(self.focus) {
            Some(li) => {
                self.lists[li].cursor = 0;
                self.preview_scroll = 0;
            }
            None => self.files.move_to_top(),
        }
    }

    fn move_to_bottom(&mut self) {
        match Self::list_index(self.focus) {
            Some(li) => {
                self.lists[li].cursor = self.len(self.focus).saturating_sub(1);
                self.preview_scroll = 0;
            }
            None => self.files.move_to_bottom(),
        }
    }
}

/// Tests of the screen.
#[cfg(test)]
mod tests;
