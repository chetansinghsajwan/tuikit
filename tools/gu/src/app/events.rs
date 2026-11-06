//! Keys and mouse: what each does in each mode.

use super::*;

impl App {
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
            Mode::Popup(p) => {
                self.popup_key(p, key);
                return Quit::No;
            }
            Mode::Normal => {}
        }
        if self.screen == Screen::Commits && self.commit_files.is_some() {
            return self.commit_files_key(key);
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
        let mut moved = true;
        match id {
            "tab.next" => self.set_screen(match self.screen {
                Screen::Status => Screen::Commits,
                Screen::Commits => Screen::Refs,
                Screen::Refs => Screen::Status,
            }),
            "tab.prev" => self.set_screen(match self.screen {
                Screen::Status => Screen::Refs,
                Screen::Commits => Screen::Status,
                Screen::Refs => Screen::Commits,
            }),
            "search.start" => self.mode = Mode::Prompt(PromptKind::Search, Input::new(self.search.clone().unwrap_or_default())),
            "search.next" => self.search_step(1),
            "search.prev" => self.search_step(-1),
            "view.detail" => self.detail = !self.detail,
            "filter.open" => self.open_filter(),
            "filter.clear" => {
                self.filter = Filter::default();
                self.refilter("no filters");
            }
            "focus.next" => self.focus_by(1),
            "focus.prev" => self.focus_by(-1),
            "focus.files" => self.show(Focus::Files),
            "focus.branches" => self.show(Focus::Branches),
            "focus.commits" => self.show(Focus::Commits),
            "focus.stashes" => self.show(Focus::Stashes),
            "back" => {
                // Esc: clear a search first, then go back to the Status tab.
                if self.work {
                    self.work = false;
                } else if self.compare.is_some() {
                    self.compare = None;
                } else if self.search.is_some() {
                    self.search = None;
                } else {
                    self.set_screen(Screen::Status);
                }
            }
            "quit" => return Quit::Yes,
            "help" => self.help = Some(ui::keys::Help::default()),
            "palette" => return Quit::Palette,
            "switch.to" => {
                let names = self.branches.iter().map(|b| b.name.clone()).collect();
                self.suggest = ui::prompt::Suggest::new(names);
                self.mode = Mode::Prompt(PromptKind::SwitchTo, Input::default());
            }
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
            "view.hash" => self.cols.hash = !self.cols.hash,
            "view.author" => self.cols.author = !self.cols.author,
            "view.author_name" => self.cols.author_name = !self.cols.author_name,
            "view.refs" => self.cols.refs = !self.cols.refs,
            "view.subject" => self.cols.subject = !self.cols.subject,
            "view.rel_time" => self.cols.rel_time = !self.cols.rel_time,
            "view.abs_time" => self.cols.abs_time = !self.cols.abs_time,
            "view.all" => {
                self.all_branches = !self.all_branches;
                self.reload_history();
                self.say(if self.all_branches { "every branch" } else { "the checked-out branch" }, false);
            }
            "preview.down" => self.pscroll += (self.pheight / 2).max(1),
            "preview.up" => self.pscroll = self.pscroll.saturating_sub((self.pheight / 2).max(1)),
            "preview.next_hunk" => self.jump_diff(true, false),
            "preview.prev_hunk" => self.jump_diff(false, false),
            "preview.next_file" => self.jump_diff(true, true),
            "preview.prev_file" => self.jump_diff(false, true),
            "preview.fold" => self.fold_file(),
            "preview.whitespace" => {
                let on = !pv::diff_ignoring_whitespace();
                pv::diff_ignore_whitespace(on);
                self.preview = None;
                self.say(if on { "ignoring whitespace changes" } else { "showing whitespace changes" }, false);
            }
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
    pub(super) fn remote(&mut self, label: &str, done: &str, f: impl FnOnce(&Repo) -> Result<(), git_core::Error> + Send + 'static) {
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
    pub(super) fn operation_name(&self) -> Option<&'static str> {
        match self.repo.operation()? {
            Operation::Merge => Some("MERGING"),
            Operation::Rebase => Some("REBASING"),
            Operation::CherryPick => Some("CHERRY-PICKING"),
            Operation::Revert => Some("REVERTING"),
        }
    }

    pub(super) fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
        self.pscroll = 0;
    }

    /// Move the focus to the next or previous pane of the tab (only the Refs tab has two).
    pub(super) fn focus_by(&mut self, delta: isize) {
        let panes: &[Focus] = match self.screen {
            Screen::Status => &STATUS_PANES,
            Screen::Commits => &COMMITS_PANES,
            Screen::Refs => &REFS_PANES,
        };
        let at = panes.iter().position(|f| *f == self.focus).unwrap_or(0) as isize;
        self.focus = panes[(at + delta).rem_euclid(panes.len() as isize) as usize];
        self.pscroll = 0;
    }

    pub(super) fn files_event(&mut self, ev: &Event) -> Quit {
        let action = self.files.handle_event(ev);
        self.files_action(action)
    }

    pub(super) fn files_action(&mut self, action: PanelAction) -> Quit {
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
    pub(super) fn pane_action(&mut self, id: &str) {
        match self.focus {
            Focus::Branches => {
                // Renaming a folder renames everything under it (a remote folder renames the remote itself).
                if id == "branch.rename" {
                    if let Some(RefRow::Folder { key, depth, .. }) = self.ref_rows.get(self.lists[0].cursor).cloned() {
                        let (section, path) = key.split_once('/').unwrap_or(("", &key));
                        match section {
                            "b" => self.mode = Mode::Prompt(PromptKind::RenameFolder { kind: RefKind::Branch, prefix: path.to_string() }, Input::new(path.to_string())),
                            "t" => self.mode = Mode::Prompt(PromptKind::RenameFolder { kind: RefKind::Tag, prefix: path.to_string() }, Input::new(path.to_string())),
                            _ if depth == 1 => self.mode = Mode::Prompt(PromptKind::RenameRemote(path.to_string()), Input::new(path.to_string())),
                            _ => self.say("rename a remote branch itself, a remote folder is only part of its name", true),
                        }
                        return;
                    }
                    if matches!(self.ref_rows.get(self.lists[0].cursor), Some(RefRow::Section { .. })) {
                        self.say("pick a ref or a folder to rename", true);
                        return;
                    }
                }
                // On a section or folder the Enter key folds or unfolds it.
                if let Some(RefRow::Section { key, .. } | RefRow::Folder { key, .. }) = self.ref_rows.get(self.lists[0].cursor) {
                    if id == "branch.checkout" {
                        let key = key.clone();
                        if !self.folded.remove(&key) {
                            self.folded.insert(key);
                        }
                        let keep = self.ref_row_id(self.lists[0].cursor);
                        self.build_ref_rows(keep);
                        return;
                    }
                }
                let Some(b) = self.current_ref().cloned() else {
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
                        let kind = match b.kind {
                            RefKind::Branch => PromptKind::RenameBranch(b.name.clone()),
                            RefKind::Tag => PromptKind::RenameTag(b.name.clone()),
                            RefKind::Remote => PromptKind::RenameRemoteBranch(b.name.clone()),
                        };
                        // A remote branch is renamed without its remote (`topic`, not `origin/topic`).
                        let shown = if b.kind == RefKind::Remote { b.name.split_once('/').map(|(_, n)| n.to_string()).unwrap_or_default() } else { b.name };
                        self.mode = Mode::Prompt(kind, Input::new(shown));
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
                    "commit.copy_hash" => match ui::clipboard::copy(&k.oid) {
                        Ok(()) => self.say(format!("copied {} to the clipboard", k.short), false),
                        Err(e) => self.say(format!("could not copy: {e}"), true),
                    },
                    "commit.files" => self.open_commit_files(),
                    "commit.compare_work" => {
                        self.work = !self.work;
                        self.compare = None;
                        self.detail = true;
                        self.say(if self.work { "diff from the commit to the work tree (M or Esc clears)" } else { "compare cleared" }, false);
                    }
                    "commit.compare" => {
                        self.work = false;
                        if self.compare.as_deref() == Some(k.oid.as_str()) {
                            self.compare = None;
                            self.say("compare cleared", false);
                        } else {
                            self.compare = Some(k.oid.clone());
                            self.detail = true;
                            self.say(format!("comparing with {}: move to another commit (m clears)", k.short), false);
                        }
                    }
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
                        self.mode = Mode::Prompt(PromptKind::Reword(k.oid), Input::new(k.subject));
                    }
                    "commit.amend" => {
                        let head = self.commits.first().map(|c| c.subject.clone()).unwrap_or_default();
                        self.mode = Mode::Prompt(PromptKind::Amend(head.clone()), Input::new(head));
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

    /// The filter popup: message, authors, categories, branches and date, with what is set chosen.
    pub(super) fn open_filter(&mut self) {
        use ui::multi::{Kind, Multi, Section};
        // The authors and categories of the history, the most frequent first.
        let (mut authors, mut cats): (Vec<(usize, String)>, Vec<(usize, String)>) = (Vec::new(), Vec::new());
        let bump = |list: &mut Vec<(usize, String)>, name: String| match list.iter_mut().find(|(_, n)| *n == name) {
            Some((n, _)) => *n += 1,
            None => list.push((1, name)),
        };
        for k in self.repo.commits_all(TAB_LOG_LIMIT).unwrap_or_default() {
            bump(&mut authors, k.author.clone());
            if let Some(w) = ui::subject::category_word(&k.subject) {
                bump(&mut cats, w);
            }
        }
        let ranked = |mut v: Vec<(usize, String)>| -> Vec<String> {
            v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            v.into_iter().map(|(_, n)| n).collect()
        };
        let f = &self.filter;
        let sections = vec![
            Section::new("Message", Kind::Text, Vec::new(), if f.text.is_empty() { Vec::new() } else { vec![f.text.clone()] }),
            Section::new("Author", Kind::Many, ranked(authors), f.authors.clone()),
            Section::new("Category", Kind::Many, ranked(cats), f.categories.clone()),
            Section::new("Branch", Kind::Many, self.branches.iter().map(|b| b.name.clone()).collect(), f.branches.clone()),
            Section::new("Since", Kind::One, ["1d", "3d", "7d", "2w", "1m", "3m", "6m", "1y"].map(String::from).to_vec(), f.since.iter().map(|(s, _)| s.clone()).collect()),
        ];
        self.mode = Mode::Popup(Multi::new("Filter commits", sections));
    }

    pub(super) fn popup_key(&mut self, mut popup: ui::multi::Multi, key: &KeyEvent) {
        match popup.key(key) {
            ui::multi::Outcome::Open => self.mode = Mode::Popup(popup),
            ui::multi::Outcome::Cancel => {}
            ui::multi::Outcome::Apply => {
                self.filter.text = popup.chosen("Message").first().cloned().unwrap_or_default();
                self.filter.authors = popup.chosen("Author").to_vec();
                self.filter.categories = popup.chosen("Category").to_vec();
                self.filter.branches = popup.chosen("Branch").to_vec();
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
                self.filter.since = popup.chosen("Since").first().and_then(|s| parse_since(s, now).map(|t| (s.clone(), t)));
                self.refilter("filtered");
            }
        }
    }

    pub(super) fn prompt_key(&mut self, kind: PromptKind, mut input: Input, ev: &Event, key: &KeyEvent) {
        // Filter prompts offer options: up and down choose, tab fills the line in.
        if matches!(kind, PromptKind::SwitchTo) {
            let ctrl = key.modifiers.contains(ratatui::crossterm::event::KeyModifiers::CONTROL);
            let step = match key.code {
                KeyCode::Down | KeyCode::Tab => Some(1),
                KeyCode::Up | KeyCode::BackTab => Some(-1),
                KeyCode::Char('n') if ctrl => Some(1),
                KeyCode::Char('p') if ctrl => Some(-1),
                _ => None,
            };
            if let Some(d) = step {
                self.suggest.typed(input.value());
                if matches!(key.code, KeyCode::Tab) {
                    if let Some(s) = self.suggest.current() {
                        input = Input::new(s);
                    }
                } else {
                    self.suggest.step(d);
                }
                self.mode = Mode::Prompt(kind, input);
                return;
            }
        }
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
                    PromptKind::RenameTag(old) => {
                        let v = value.clone();
                        self.run(format!("renamed {old} to {value}"), move |r| r.rename_tag(&old, &v));
                    }
                    PromptKind::RenameRemote(old) => {
                        let v = value.clone();
                        self.run(format!("renamed remote {old} to {value}"), move |r| r.rename_remote(&old, &v));
                    }
                    PromptKind::RenameRemoteBranch(old) => {
                        let v = value.clone();
                        let done = format!("renamed {old} to {value} on the remote");
                        self.remote("renaming on the remote", &done, move |r| r.rename_remote_branch(&old, &v));
                    }
                    PromptKind::RenameFolder { kind, prefix } => {
                        let value = value.trim_end_matches('/').to_string();
                        let pairs: Vec<(String, String)> = self.branches.iter().filter(|r| r.kind == kind && r.name.starts_with(&format!("{prefix}/"))).map(|r| (r.name.clone(), format!("{value}{}", &r.name[prefix.len()..]))).collect();
                        let n = pairs.len();
                        // The names are checked and the renames are all or none (`Repo::rename_many`).
                        if value.is_empty() || value.starts_with('/') || value.contains("//") {
                            self.say("the folder name is empty or has an empty part", true);
                        } else {
                            self.run(format!("renamed {n} ref{} to {value}/", if n == 1 { "" } else { "s" }), move |r| r.rename_many(kind, &pairs));
                        }
                    }
                    PromptKind::BranchAt(oid) => {
                        let v = value.clone();
                        self.run(format!("created {value}"), move |r| r.create_branch_at(&v, &oid));
                    }
                    PromptKind::TagAt(oid) => {
                        let v = value.clone();
                        self.run(format!("tagged {value}"), move |r| r.create_tag(&v, &oid));
                    }
                    PromptKind::Reword(oid) => {
                        let v = value.clone();
                        self.run("reworded".to_string(), move |r| r.reword_commit(&oid, &v));
                    }
                    PromptKind::SwitchTo => self.switch_to(value),
                    PromptKind::Amend(old) => {
                        let v = value.clone();
                        self.run("amended the last commit".to_string(), move |r| if v == old { r.amend(None) } else { r.amend(Some(&v)) });
                    }
                }
            }
            _ => {
                input.handle_event(ev);
                self.suggest.typed(input.value());
                self.mode = Mode::Prompt(kind, input);
            }
        }
    }

    /// Check out what was typed: a ref by its name, the option chosen from the list, or a commit.
    pub(super) fn switch_to(&mut self, typed: String) {
        let typed = typed.trim().to_string();
        let by = |name: &str| self.branches.iter().find(|b| b.name == name).map(|b| (b.name.clone(), b.kind, b.is_head));
        let pick = by(&typed).or_else(|| self.suggest.current().and_then(|s| by(&s)));
        match pick {
            Some((name, _, true)) => self.say(format!("already on {name}"), false),
            Some((name, kind, false)) => self.run(format!("switched to {name}"), move |r| r.checkout_ref(&name, kind)),
            None if typed.is_empty() => {}
            None => match self.repo.commit(&typed) {
                Ok(k) => {
                    let (oid, short) = (k.oid.clone(), k.short.clone());
                    self.run(format!("checked out {short}"), move |r| r.checkout_commit(&oid));
                }
                Err(_) => self.say(format!("no branch, tag or commit named {typed}"), true),
            },
        }
    }

    pub(super) fn confirm_key(&mut self, c: Confirm, key: &KeyEvent) {
        let again = matches!(c, Confirm::DeleteBranch { .. } | Confirm::DeleteTag(_) | Confirm::DeleteRemote(_) | Confirm::DropStash(_));
        if !(matches!(key.code, KeyCode::Char('y' | 'Y') | KeyCode::Enter) || (again && matches!(key.code, KeyCode::Char('d' | 'D')))) {
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
}
