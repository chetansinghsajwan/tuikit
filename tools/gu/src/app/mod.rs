use git_core::{
    CommitInfo, Operation, RefInfo, RefKind, Repo, ResetMode, StashInfo, SubmoduleInfo,
};
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

mod data;
mod events;
mod pills;
mod refs;
mod render;
mod search;
mod types;
use pills::*;
pub use types::Quit;
use types::*;

#[cfg(test)]
mod tests;

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
    pscroll: usize,
    /// The commit whose files were folded by hand (another commit starts with none).
    fold_rev: Option<String>,
    pheight: usize,
    /// The keymaps of the Branches, Commits, Stashes and Submodules panes (the Files pane has its own).
    maps: [Keymap; 4],
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

        let mut files =
            Panel::with_actions(dir, "space", crate::keys::GLOBAL.to_vec(), crate::keys::GROUPS)?;
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
            pscroll: 0,
            fold_rev: None,
            pheight: 10,
            maps: [0, 1, 2, 3]
                .map(|i| Keymap::new("space", &crate::keys::pane_actions(i), crate::keys::GROUPS)),
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
