//! Tabs, focus and search.

use super::*;

impl App {
    /// The tab that shows `focus`.
    pub(super) fn tab_of(focus: Focus) -> Tab {
        match focus {
            Focus::Files => Tab::Status,
            Focus::Commits => Tab::Commits,
            Focus::Branches | Focus::Stashes | Focus::Submodules => Tab::Refs,
        }
    }

    /// Switch tab (and focus its first pane).
    pub(super) fn select_tab(&mut self, tab: Tab) {
        if tab == self.tab {
            return;
        }
        self.tab = tab;
        self.focus = match tab {
            Tab::Status => Focus::Files,
            Tab::Commits => Focus::Commits,
            Tab::Refs => Focus::Branches,
        };
        if tab != Tab::Commits {
            self.search = None;
        }
        self.preview_scroll.reset();
        // The commits are only loaded while their tab is shown.
        self.reload_history();
    }

    /// Show `focus`'s pane: its tab, and it focused.
    pub(super) fn show(&mut self, focus: Focus) {
        self.select_tab(Self::tab_of(focus));
        self.set_focus(focus);
    }

    /// Reload the list after a filter changed and say how many commits are left.
    pub(super) fn refilter(&mut self, what: &str) {
        self.lists[1] = ListState::default();
        self.preview_scroll.reset();
        self.reload_history();
        let n = self.commits.len();
        let shown = if self.filter.active() {
            format!("{what}: {n} commit{}", if n == 1 { "" } else { "s" })
        } else {
            what.to_string()
        };
        self.show_message(shown, false);
    }

    /// Whether commit `i` of the list matches the search.
    pub(super) fn commit_matches(&self, i: usize) -> bool {
        let Some(q) = self.search.as_deref().filter(|q| !q.is_empty()) else { return false };
        let q = q.to_lowercase();
        let Some(k) = self.commits.get(i) else { return false };
        let refs = self
            .labels
            .get(&k.oid)
            .into_iter()
            .flatten()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        [k.oid.as_str(), k.subject.as_str(), k.author.as_str(), refs.as_str()]
            .iter()
            .any(|s| s.to_lowercase().contains(&q))
    }

    /// Go to the next (`step` 1) or previous (-1) commit matching the search, wrapping around.
    pub(super) fn search_step(&mut self, step: isize) {
        let n = self.commits.len();
        if self.search.is_none() || n == 0 {
            self.show_message("no search: / starts one", true);
            return;
        }
        let from = self.lists[1].cursor;
        let found = (1..=n)
            .map(|d| (from as isize + step * d as isize).rem_euclid(n as isize) as usize)
            .find(|&i| self.commit_matches(i));
        let total = (0..n).filter(|&i| self.commit_matches(i)).count();
        match found {
            Some(i) => {
                self.lists[1].cursor = i;
                self.preview_scroll.reset();
                let nth = (0..=i).filter(|&j| self.commit_matches(j)).count();
                self.show_message(format!("match {nth} of {total}"), false);
            }
            None => self.show_message(
                format!("no commit matches \"{}\"", self.search.clone().unwrap_or_default()),
                true,
            ),
        }
    }

    /// The actions that start the search or step through its matches.
    pub(super) fn search_action(&mut self, id: &str) -> bool {
        match id {
            "search.start" => {
                self.mode = Mode::Prompt(
                    PromptKind::Search,
                    Input::new(self.search.clone().unwrap_or_default()),
                )
            }
            "search.next" => self.search_step(1),
            "search.prev" => self.search_step(-1),
            _ => return false,
        }
        true
    }

    /// Apply a finished search prompt.
    pub(super) fn search_submit(&mut self, kind: PromptKind, value: String) {
        match kind {
            PromptKind::Search => {
                self.search = if value.is_empty() { None } else { Some(value) };
                if self.search.is_some() {
                    // The commit under the cursor counts if it matches.
                    let at = self.lists[1].cursor;
                    if self.commit_matches(at) {
                        let total =
                            (0..self.commits.len()).filter(|&i| self.commit_matches(i)).count();
                        let nth = (0..=at).filter(|&j| self.commit_matches(j)).count();
                        self.show_message(format!("match {nth} of {total}"), false);
                    } else {
                        self.search_step(1);
                    }
                }
            }
            _ => {}
        }
    }
}
