//! Tabs, focus and search.

use super::*;

impl App {
    /// The tab that shows `focus`.
    pub(super) fn screen_of(focus: Focus) -> Screen {
        match focus {
            Focus::Files => Screen::Status,
            Focus::Commits => Screen::Commits,
            Focus::Branches | Focus::Stashes => Screen::Refs,
        }
    }

    /// Switch tab (and focus its first pane).
    pub(super) fn set_screen(&mut self, screen: Screen) {
        if screen == self.screen {
            return;
        }
        self.screen = screen;
        self.focus = match screen {
            Screen::Status => Focus::Files,
            Screen::Commits => Focus::Commits,
            Screen::Refs => Focus::Branches,
        };
        if screen != Screen::Commits {
            self.search = None;
        }
        self.pscroll = 0;
        // The commits are only loaded while their tab is shown.
        self.reload_history();
    }

    /// Show `focus`'s pane: its tab, and it focused.
    pub(super) fn show(&mut self, focus: Focus) {
        self.set_screen(Self::screen_of(focus));
        self.set_focus(focus);
    }

    /// Reload the list after a filter changed and say how many commits are left.
    pub(super) fn refilter(&mut self, what: &str) {
        self.lists[1] = ListState::default();
        self.pscroll = 0;
        self.reload_history();
        let n = self.commits.len();
        let shown = if self.filter.active() {
            format!("{what}: {n} commit{}", if n == 1 { "" } else { "s" })
        } else {
            what.to_string()
        };
        self.say(shown, false);
    }

    /// Whether commit `i` of the list matches the search.
    pub(super) fn commit_matches(&self, i: usize) -> bool {
        let Some(q) = self.search.as_deref().filter(|q| !q.is_empty()) else { return false };
        let q = q.to_lowercase();
        let Some(k) = self.commits.get(i) else { return false };
        let refs = self.labels.get(&k.oid).into_iter().flatten().map(|r| r.name.as_str()).collect::<Vec<_>>().join(" ");
        [k.oid.as_str(), k.subject.as_str(), k.author.as_str(), refs.as_str()]
            .iter()
            .any(|s| s.to_lowercase().contains(&q))
    }

    /// Go to the next (`step` 1) or previous (-1) commit matching the search, wrapping around.
    pub(super) fn search_step(&mut self, step: isize) {
        let n = self.commits.len();
        if self.search.is_none() || n == 0 {
            self.say("no search: / starts one", true);
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
                self.pscroll = 0;
                let nth = (0..=i).filter(|&j| self.commit_matches(j)).count();
                self.say(format!("match {nth} of {total}"), false);
            }
            None => self.say(format!("no commit matches \"{}\"", self.search.clone().unwrap_or_default()), true),
        }
    }
}
