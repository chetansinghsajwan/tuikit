//! Loading: history, status, refs and the commit list.

use super::*;

impl App {
    pub(super) fn reload_history(&mut self) {
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
        refs.sort_by(|a, b| {
            rank(a).cmp(&rank(b)).then(b.is_head.cmp(&a.is_head)).then(b.time.cmp(&a.time))
        });
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
            all.sort_by_key(|a| std::cmp::Reverse(a.time));
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
        self.graph_w =
            self.graph_rows.iter().map(|r| r.cells.len()).max().unwrap_or(0).min(GRAPH_MAX);
        self.labels.clear();
        for r in &self.branches {
            self.labels.entry(r.oid.clone()).or_default().push(r.clone());
        }
        self.author_w = AUTHOR_COL;
        // Every column has one width, whatever the commits are: the refs column too (when any commit has a ref).
        self.refs_need = self
            .commits
            .iter()
            .filter_map(|k| self.labels.get(&k.oid))
            .map(|l| pills(l).iter().map(|(r, remotes)| ref_width(r, remotes)).sum::<usize>())
            .max()
            .unwrap_or(0);
        self.refs_w = self.refs_need.min(REFS_COL);
        self.stashes = self.repo.stashes().unwrap_or_default();
        self.submodules = self.repo.submodules().unwrap_or_default();
        self.build_ref_rows(keep);
        for (i, len) in
            [self.ref_rows.len(), self.commits.len(), self.stashes.len(), self.submodules.len()]
                .into_iter()
                .enumerate()
        {
            self.lists[i].cursor = self.lists[i].cursor.min(len.saturating_sub(1));
        }
        self.preview = None;
    }

    pub(super) fn reload_all(&mut self) {
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

    pub(super) fn say(&mut self, msg: impl Into<String>, error: bool) {
        self.message = Some((msg.into(), error));
    }

    pub(super) fn list_index(focus: Focus) -> Option<usize> {
        match focus {
            Focus::Files => None,
            Focus::Branches => Some(0),
            Focus::Commits => Some(1),
            Focus::Stashes => Some(2),
            Focus::Submodules => Some(3),
        }
    }

    pub(super) fn len(&self, focus: Focus) -> usize {
        match focus {
            Focus::Files => 0,
            Focus::Branches => self.ref_rows.len(),
            Focus::Commits => self.commits.len(),
            Focus::Stashes => self.stashes.len(),
            Focus::Submodules => self.submodules.len(),
        }
    }

    /// Run a git change, report the outcome, reload everything.
    pub(super) fn run(&mut self, ok: String, f: impl FnOnce(&Repo) -> Result<(), git_core::Error>) {
        match f(&self.repo) {
            Ok(()) => self.say(ok, false),
            Err(e) => self.say(e.to_string(), true),
        }
        self.reload_all();
    }
}
