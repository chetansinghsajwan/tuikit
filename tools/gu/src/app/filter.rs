//! The commit filter popup.

use super::*;

impl App {
    /// The filter popup: message, authors, categories, branches and date, with what is set chosen.
    pub(super) fn open_filter(&mut self) {
        use ui::multi::{Kind, Multi, Section};
        // The authors and categories of the history, the most frequent first.
        /// How often each author or category appears, to list the common ones first.
        type Counts = Vec<(usize, String)>;
        let (mut authors, mut cats): (Counts, Counts) = (Vec::new(), Vec::new());
        let bump = |list: &mut Vec<(usize, String)>, name: String| match list
            .iter_mut()
            .find(|(_, n)| *n == name)
        {
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
            Section::new(
                "Message",
                Kind::Text,
                Vec::new(),
                if f.text.is_empty() { Vec::new() } else { vec![f.text.clone()] },
            ),
            Section::new("Author", Kind::Many, ranked(authors), f.authors.clone()),
            Section::new("Category", Kind::Many, ranked(cats), f.categories.clone()),
            Section::new(
                "Branch",
                Kind::Many,
                self.branches.iter().map(|b| b.name.clone()).collect(),
                f.branches.clone(),
            ),
            Section::new(
                "Since",
                Kind::One,
                ["1d", "3d", "7d", "2w", "1m", "3m", "6m", "1y"].map(String::from).to_vec(),
                f.since.iter().map(|(s, _)| s.clone()).collect(),
            ),
        ];
        self.mode = Mode::Popup(Multi::new("Filter commits", sections));
    }

    /// A key in the filter popup: keep it open, drop it, or apply the choices as the commit filter.
    pub(super) fn popup_key(&mut self, mut popup: ui::multi::Multi, key: &KeyEvent) {
        match popup.key(key) {
            ui::multi::Outcome::Open => self.mode = Mode::Popup(popup),
            ui::multi::Outcome::Cancel => {}
            ui::multi::Outcome::Apply => {
                self.filter.text = popup.chosen("Message").first().cloned().unwrap_or_default();
                self.filter.authors = popup.chosen("Author").to_vec();
                self.filter.categories = popup.chosen("Category").to_vec();
                self.filter.branches = popup.chosen("Branch").to_vec();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                self.filter.since = popup
                    .chosen("Since")
                    .first()
                    .and_then(|s| parse_since(s, now).map(|t| (s.clone(), t)));
                self.refilter("filtered");
            }
        }
    }

    /// The actions that open or clear the filter.
    pub(super) fn filter_action(&mut self, id: &str) -> bool {
        match id {
            "filter.open" => self.open_filter(),
            "filter.clear" => {
                self.filter = Filter::default();
                self.refilter("no filters");
            }
            _ => return false,
        }
        true
    }
}
