//! The kinds of things the screen holds: focus, modes, prompts, rows, filters.

use super::*;

/// How many commits the Commits tab loads.
pub(super) const TAB_LOG_LIMIT: usize = 3000;
/// Most cells (two per lane) the graph column may take.
pub(super) const GRAPH_MAX: usize = 24;
/// Most cells the refs column of the Commits table may take.
pub(super) const REFS_COL: usize = 24;

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
pub(super) enum Focus {
    Files,
    Branches,
    Commits,
    Stashes,
    Submodules,
}

/// The panes of each tab, in focus order.
pub(super) const STATUS_PANES: [Focus; 1] = [Focus::Files];
pub(super) const COMMITS_PANES: [Focus; 1] = [Focus::Commits];
pub(super) const REFS_PANES: [Focus; 3] = [Focus::Branches, Focus::Stashes, Focus::Submodules];

pub(super) enum PromptKind {
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
    RenameFolder {
        kind: RefKind,
        prefix: String,
    },
    /// A branch at this commit.
    BranchAt(String),
    /// A tag at this commit.
    TagAt(String),
    /// A new message for the last commit.
    Reword(String),
    /// Check out a branch, tag, remote branch or commit by name.
    SwitchTo,
    /// Add a submodule: the URL, and optionally the path after a space.
    AddSubmodule,
    /// Amend the last commit with what is staged, with this message (the old one is the value).
    Amend(String),
}

pub(super) enum Confirm {
    DeleteBranch {
        name: String,
        force: bool,
    },
    Merge(String),
    DropStash(usize),
    Rebase(String),
    Revert(String),
    Reset {
        oid: String,
        mode: ResetMode,
    },
    ForcePush,
    /// Delete a tag.
    DeleteTag(String),
    /// Delete a branch on its remote (`origin/topic`).
    DeleteRemote(String),
    /// Remove a submodule (by path): its work tree goes too.
    RemoveSubmodule(String),
}

/// A git command talking to a remote, running on its own thread.
pub(super) struct Job {
    pub(super) label: String,
    pub(super) done: String,
    pub(super) handle: std::thread::JoinHandle<Result<(), git_core::Error>>,
}

/// The tabs: the changed files, the full commit graph, and the refs (branches, tags, stashes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Screen {
    Status,
    Commits,
    Refs,
}

pub(super) enum Mode {
    Normal,
    Prompt(PromptKind, Input),
    Confirm(Confirm),
    /// The filter popup.
    Popup(ui::multi::Multi),
}

/// A row of the Refs tree: a section (Branches, Remotes, Tags), a folder (`feature/`, `origin/`) or a ref.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RefRow {
    Section {
        key: String,
        label: &'static str,
        count: usize,
    },
    Folder {
        key: String,
        name: String,
        depth: usize,
        count: usize,
    },
    /// `index` into `branches`; `name` is the last path part.
    Item {
        index: usize,
        name: String,
        depth: usize,
    },
}

/// Which columns of the Commits table show (`v` menu).
#[derive(Clone, Copy)]
pub(super) struct Columns {
    pub(super) hash: bool,
    pub(super) author: bool,
    /// Authors as full names instead of two initials.
    pub(super) author_name: bool,
    pub(super) refs: bool,
    pub(super) subject: bool,
    pub(super) rel_time: bool,
    pub(super) abs_time: bool,
}

impl Default for Columns {
    fn default() -> Self {
        Columns {
            hash: true,
            author: true,
            author_name: false,
            refs: true,
            subject: true,
            rel_time: true,
            abs_time: false,
        }
    }
}

/// The files of one commit, as a folder tree (Commits tab, `l` on a commit); the diff of the one under the
/// cursor shows beside it.
pub(super) struct CommitFiles {
    pub(super) oid: String,
    pub(super) short: String,
    pub(super) subject: String,
    pub(super) changes: Vec<git_core::FileChange>,
    pub(super) rows: Vec<FileRow>,
    pub(super) cursor: usize,
    pub(super) offset: usize,
    pub(super) folded: std::collections::HashSet<String>,
}

pub(super) enum FileRow {
    Folder { path: String, label: String, depth: usize, count: usize, folded: bool },
    File { change: git_core::FileChange, depth: usize },
}

impl FileRow {
    pub(super) fn path(&self) -> &str {
        match self {
            FileRow::Folder { path, .. } => path,
            FileRow::File { change, .. } => &change.path,
        }
    }
}

#[derive(Default)]
pub(super) struct FileNode {
    pub(super) dirs: std::collections::BTreeMap<String, FileNode>,
    pub(super) files: Vec<git_core::FileChange>,
}

impl FileNode {
    pub(super) fn count(&self) -> usize {
        self.files.len() + self.dirs.values().map(FileNode::count).sum::<usize>()
    }
}

impl CommitFiles {
    pub(super) fn new(
        oid: String,
        short: String,
        subject: String,
        mut changes: Vec<git_core::FileChange>,
    ) -> CommitFiles {
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        let mut cf = CommitFiles {
            oid,
            short,
            subject,
            changes,
            rows: Vec::new(),
            cursor: 0,
            offset: 0,
            folded: Default::default(),
        };
        cf.rebuild();
        cf
    }

    pub(super) fn rebuild(&mut self) {
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
        fn push(
            node: &FileNode,
            prefix: &str,
            depth: usize,
            folded: &std::collections::HashSet<String>,
            rows: &mut Vec<FileRow>,
        ) {
            for (name, child) in &node.dirs {
                let (mut label, mut child, mut path) = (
                    name.clone(),
                    child,
                    if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") },
                );
                // A folder with nothing but one folder in it joins it: `src/lib`.
                while child.files.is_empty() && child.dirs.len() == 1 {
                    let (n, c) = child.dirs.iter().next().expect("one folder");
                    label = format!("{label}/{n}");
                    path = format!("{path}/{n}");
                    child = c;
                }
                let is_folded = folded.contains(&path);
                rows.push(FileRow::Folder {
                    path: path.clone(),
                    label,
                    depth,
                    count: child.count(),
                    folded: is_folded,
                });
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
        self.cursor = keep
            .and_then(|p| self.rows.iter().position(|r| r.path() == p))
            .unwrap_or(self.cursor)
            .min(self.rows.len().saturating_sub(1));
    }

    pub(super) fn move_by(&mut self, delta: isize) {
        let last = self.rows.len().saturating_sub(1) as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, last.max(0)) as usize;
    }

    /// The path of the row under the cursor (a folder's covers every file below it).
    pub(super) fn current_path(&self) -> Option<&str> {
        self.rows.get(self.cursor).map(FileRow::path)
    }
}

/// What narrows the commit list. The graph is drawn from what is left.
#[derive(Default, Clone)]
pub(super) struct Filter {
    pub(super) text: String,
    /// Commits by any of these authors.
    pub(super) authors: Vec<String>,
    /// Only the history of these branches or tags (all of them together).
    pub(super) branches: Vec<String>,
    /// Commits whose subject starts with any of these categories (`fix`, `feat`).
    pub(super) categories: Vec<String>,
    /// As typed (`7d`) and as seconds since the epoch.
    pub(super) since: Option<(String, i64)>,
}

impl Filter {
    pub(super) fn active(&self) -> bool {
        !self.text.is_empty()
            || !self.authors.is_empty()
            || !self.branches.is_empty()
            || !self.categories.is_empty()
            || self.since.is_some()
    }

    /// The parts that are set, as `(what, value)` for the filter bar and the popup.
    pub(super) fn parts(&self) -> Vec<(&'static str, String)> {
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

    pub(super) fn keeps(&self, c: &CommitInfo) -> bool {
        (self.text.is_empty() || c.message.to_lowercase().contains(&self.text.to_lowercase()))
            && (self.authors.is_empty()
                || self.authors.iter().any(|a| a.eq_ignore_ascii_case(&c.author)))
            && (self.categories.is_empty()
                || ui::subject::category_word(&c.subject)
                    .is_some_and(|w| self.categories.contains(&w)))
            && self.since.as_ref().is_none_or(|(_, t)| c.time >= *t)
    }
}

/// Seconds since the epoch for `7d`, `2w`, `3m` (30 days), `1y` or `2026-10-01`; `now` is the current time.
pub(super) fn parse_since(text: &str, now: i64) -> Option<i64> {
    let text = text.trim();
    let (digits, unit) =
        text.split_at(text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len()));
    if let (Ok(n), Some(secs)) = (
        digits.parse::<i64>(),
        match unit {
            "d" => Some(86_400),
            "w" => Some(7 * 86_400),
            "m" => Some(30 * 86_400),
            "y" => Some(365 * 86_400),
            _ => None,
        },
    ) {
        return Some(now - n * secs);
    }
    // A date: days since 1970-01-01 by the civil-calendar formula.
    let mut it = text.split('-').map(|p| p.parse::<i64>());
    let (Some(Ok(y)), Some(Ok(m)), Some(Ok(d)), None) =
        (it.next(), it.next(), it.next(), it.next())
    else {
        return None;
    };
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
pub(super) struct ListState {
    pub(super) cursor: usize,
    pub(super) offset: usize,
}

pub(super) struct Preview {
    pub(super) key: (Focus, String, u16),
    pub(super) lines: Vec<Line<'static>>,
    /// Where the files and hunks of the patch start (for the jump keys).
    pub(super) anchors: pv::Anchors,
}
