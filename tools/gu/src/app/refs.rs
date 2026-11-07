//! The Refs tab's tree of branches, remotes, tags and stashes.

use super::*;

impl App {
    /// What identifies row `i` of the Refs tree across reloads: the key of a section or folder, the kind and name of a ref.
    pub(super) fn ref_row_id(&self, i: usize) -> Option<String> {
        match self.ref_rows.get(i)? {
            RefRow::Section { key, .. } | RefRow::Folder { key, .. } => Some(key.clone()),
            RefRow::Item { index, .. } => self.branches.get(*index).map(|r| format!("{:?}:{}", r.kind, r.name)),
        }
    }

    /// The ref under the cursor of the Refs pane (`None` on a section or a folder).
    pub(super) fn current_ref(&self) -> Option<&RefInfo> {
        match self.ref_rows.get(self.lists[0].cursor)? {
            RefRow::Item { index, .. } => self.branches.get(*index),
            _ => None,
        }
    }

    /// Lay the refs out as a tree: Branches, Remotes and Tags, each with its names split at `/` into folders
    /// (`feature/login` under `feature`, `origin/main` under `origin`). Folded sections and folders hide what is in them.
    pub(super) fn build_ref_rows(&mut self, keep: Option<String>) {
        let mut rows = Vec::new();
        for (kind, label, tag) in
            [(RefKind::Branch, "Branches", "b"), (RefKind::Remote, "Remotes", "r"), (RefKind::Tag, "Tags", "t")]
        {
            let mut items: Vec<(usize, &RefInfo)> =
                self.branches.iter().enumerate().filter(|(_, r)| r.kind == kind).collect();
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
                    let count = self
                        .branches
                        .iter()
                        .filter(|b| b.kind == kind && b.name.starts_with(&format!("{}/", open.join("/"))))
                        .count();
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
        let at = keep
            .and_then(|k| (0..self.ref_rows.len()).find(|&i| self.ref_row_id(i).as_deref() == Some(k.as_str())))
            .unwrap_or(self.lists[0].cursor);
        self.lists[0].cursor = at.min(self.ref_rows.len().saturating_sub(1));
    }

    /// Scroll the diff to the next or previous hunk (or file).
    /// Open the file tree of the commit under the cursor.
    pub(super) fn open_commit_files(&mut self) {
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
    pub(super) fn commit_files_key(&mut self, key: &KeyEvent) -> Quit {
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
                        if let Some(i) = cf.rows[..cf.cursor]
                            .iter()
                            .rposition(|r| matches!(r, FileRow::Folder { .. }) && depth_of(r) == want)
                        {
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
    pub(super) fn fold_file(&mut self) {
        let Some(p) = &self.preview else { return };
        let at = p.anchors.files.iter().rposition(|&m| m <= self.pscroll).or(if p.anchors.files.is_empty() {
            None
        } else {
            Some(0)
        });
        if let Some(path) = at.and_then(|i| p.anchors.paths.get(i)).cloned() {
            pv::diff_toggle_fold(&path);
            self.preview = None;
        }
    }

    pub(super) fn jump_diff(&mut self, forward: bool, files: bool) {
        let Some(p) = &self.preview else { return };
        let marks = if files { &p.anchors.files } else { &p.anchors.hunks };
        let target =
            if forward { pv::Anchors::next(marks, self.pscroll) } else { pv::Anchors::prev(marks, self.pscroll) };
        if let Some(t) = target {
            self.pscroll = t;
        }
    }
}
