//! Tabs, the same in every tool: a model ([`Tabs`]) that owns the list and the active
//! tab, and a strip ([`render`]) that draws it: numbered labels, the active one bold in
//! the accent color, a marker for unsaved changes, and when the labels do
//! not fit a window around the active tab with `‹N` / `N›` for the ones out of view.
//!
//! The strip is one row. Keys come from each tool's action registry (`Space t n/c/l/h`,
//! `[` and `]`, `1`-`9`), which call the model's methods.

use crate::icons::Icon;
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

/// How many closed tabs are remembered for `reopen`.
const CLOSED_KEPT: usize = 10;

/// A list of tabs with one active.
#[derive(Debug, Clone)]
pub struct Tabs<T> {
    items: Vec<T>,
    active: usize,
    closed: Vec<T>,
}

impl<T> Tabs<T> {
    pub fn new(first: T) -> Tabs<T> {
        Tabs { items: vec![first], active: 0, closed: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The active tab's position (0-based).
    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active(&self) -> &T {
        &self.items[self.active]
    }

    pub fn active_mut(&mut self) -> &mut T {
        &mut self.items[self.active]
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    /// Add a tab after the active one and switch to it.
    pub fn open(&mut self, tab: T) {
        self.items.insert(self.active + 1, tab);
        self.active += 1;
    }

    /// Close the active tab (kept for `reopen`). The last tab cannot be closed: `None`.
    pub fn close(&mut self) -> Option<()> {
        if self.items.len() <= 1 {
            return None;
        }
        let tab = self.items.remove(self.active);
        self.closed.push(tab);
        if self.closed.len() > CLOSED_KEPT {
            self.closed.remove(0);
        }
        self.active = self.active.min(self.items.len() - 1);
        Some(())
    }

    /// Bring back the tab closed last, after the active one. `false` if there is none.
    pub fn reopen(&mut self) -> bool {
        match self.closed.pop() {
            Some(tab) => {
                self.open(tab);
                true
            }
            None => false,
        }
    }

    /// The next tab, wrapping at the end.
    pub fn next(&mut self) {
        self.active = (self.active + 1) % self.items.len();
    }

    /// The previous tab, wrapping at the start.
    pub fn prev(&mut self) {
        self.active = (self.active + self.items.len() - 1) % self.items.len();
    }

    /// Switch to tab `n` (0-based). `false` if there is no such tab.
    pub fn goto(&mut self, n: usize) -> bool {
        if n < self.items.len() {
            self.active = n;
            true
        } else {
            false
        }
    }

    /// Move the active tab `delta` places (negative: left), staying in range.
    pub fn move_active(&mut self, delta: isize) {
        let to = (self.active as isize + delta).clamp(0, self.items.len() as isize - 1) as usize;
        let tab = self.items.remove(self.active);
        self.items.insert(to, tab);
        self.active = to;
    }
}

impl<T: Clone> Tabs<T> {
    /// A copy of the active tab, next to it.
    pub fn duplicate(&mut self) {
        let copy = self.items[self.active].clone();
        self.open(copy);
    }
}

/// What the strip shows for one tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub title: String,
    /// Unsaved changes: a marker after the title.
    pub modified: bool,
    pub icon: Option<Icon>,
}

impl Label {
    pub fn new(title: impl Into<String>) -> Label {
        Label { title: title.into(), modified: false, icon: None }
    }
}

/// Titles for tabs showing `paths`: the file or folder name, and where two tabs would read
/// the same, as many parent folders as it takes to tell them apart (`src/lib.rs` and
/// `ui/lib.rs`). A path with no name (`/`) is shown as `/`.
pub fn short_titles(paths: &[&std::path::Path]) -> Vec<String> {
    let comps: Vec<Vec<String>> = paths.iter().map(|p| p.components().filter_map(|c| match c {
        std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
        _ => None,
    }).collect()).collect();
    // How many trailing components each path shows; raised for the paths that still clash.
    let mut depth = vec![1usize; paths.len()];
    let title = |i: usize, d: usize| -> String {
        let c = &comps[i];
        if c.is_empty() {
            return "/".to_string();
        }
        c[c.len().saturating_sub(d)..].join("/")
    };
    for _ in 0..64 {
        let titles: Vec<String> = (0..paths.len()).map(|i| title(i, depth[i])).collect();
        let mut changed = false;
        for i in 0..paths.len() {
            let clashes = (0..paths.len()).any(|j| j != i && titles[j] == titles[i]);
            if clashes && depth[i] < comps[i].len() {
                depth[i] += 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    (0..paths.len()).map(|i| title(i, depth[i])).collect()
}

/// The text cells of a label, `[number] [icon] title [marker]` between single spaces.
fn parts(i: usize, l: &Label, numbers: bool) -> Vec<(String, Part)> {
    let mut v = vec![(" ".to_string(), Part::Gap)];
    if numbers {
        v.push((format!("{} ", i + 1), Part::Number));
    }
    if let Some(icon) = l.icon {
        v.push((format!("{} ", icon.glyph), Part::Icon(icon)));
    }
    v.push((l.title.clone(), Part::Title));
    if l.modified {
        v.push((" ●".to_string(), Part::Marker));
    }
    v.push((" ".to_string(), Part::Gap));
    v
}

#[derive(Clone, Copy)]
enum Part {
    Gap,
    Number,
    Icon(Icon),
    Title,
    Marker,
}

fn width(parts: &[(String, Part)]) -> usize {
    parts.iter().map(|(s, _)| s.width()).sum()
}

fn spans(parts: Vec<(String, Part)>, active: bool, theme: &Theme) -> Vec<Span<'static>> {
    let c = &theme.colors;
    let base = if active { Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD) } else { Style::new().fg(c.muted.into()) };
    parts
        .into_iter()
        .map(|(text, part)| {
            let style = match part {
                Part::Gap => Style::new(),
                Part::Number => base,
                Part::Icon(i) => {
                    let s = Style::new().fg(i.color);
                    if active { s } else { s.add_modifier(Modifier::DIM) }
                }
                Part::Title => base,
                Part::Marker => Style::new().fg(c.warning.into()),
            };
            Span::styled(text, style)
        })
        .collect()
}

/// Cut a title to `room` cells with a `…`.
fn clip(title: &str, room: usize) -> String {
    if title.width() <= room {
        return title.to_string();
    }
    let mut out = String::new();
    for ch in title.chars() {
        if out.width() + 1 >= room {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

/// Which labels to show: all if they fit, else a window around the active one with room for
/// the `‹N` and `N›` markers. Returns `(first, last)` inclusive.
fn window(widths: &[usize], active: usize, room: usize) -> (usize, usize) {
    let n = widths.len();
    let marker = |k: usize| if k == 0 { 0 } else { format!("‹{k} ").width() };
    let marker_r = |k: usize| if k == 0 { 0 } else { format!(" {k}›").width() };
    let (mut lo, mut hi) = (active, active);
    let used = |lo: usize, hi: usize| widths[lo..=hi].iter().sum::<usize>() + marker(lo) + marker_r(n - 1 - hi);
    loop {
        // Grow to the right first, then to the left, while everything still fits.
        if hi + 1 < n && used(lo, hi + 1) <= room {
            hi += 1;
        } else if lo > 0 && used(lo - 1, hi) <= room {
            lo -= 1;
        } else {
            break;
        }
    }
    (lo, hi)
}

/// Draw the strip in the row `area` (only its first row is used).
pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme, labels: &[Label], active: usize, numbers: bool) {
    if labels.is_empty() || area.width < 4 || area.height == 0 {
        return;
    }
    let c = &theme.colors;
    let room = usize::from(area.width);
    let all: Vec<Vec<(String, Part)>> = labels.iter().enumerate().map(|(i, l)| parts(i, l, numbers)).collect();
    let widths: Vec<usize> = all.iter().map(|p| width(p)).collect();
    let active = active.min(labels.len() - 1);
    let (lo, hi) = if widths.iter().sum::<usize>() <= room { (0, labels.len() - 1) } else { window(&widths, active, room) };

    let mut line: Vec<Span<'static>> = Vec::new();
    let muted = Style::new().fg(c.muted.into());
    if lo > 0 {
        line.push(Span::styled(format!("‹{lo} "), muted));
    }
    for i in lo..=hi {
        let mut p = all[i].clone();
        // A single label wider than the strip is cut.
        if lo == hi && widths[i] > room {
            let extra = widths[i] - room + 1;
            if let Some((t, _)) = p.iter_mut().find(|(_, part)| matches!(part, Part::Title)) {
                let keep = t.width().saturating_sub(extra).max(1);
                *t = clip(t, keep);
            }
        }
        line.extend(spans(p, i == active, theme));
    }
    if hi + 1 < labels.len() {
        line.push(Span::styled(format!(" {}›", labels.len() - 1 - hi), muted));
    }
    Paragraph::new(Line::from(line)).render(Rect { height: 1, ..area }, buf);
    let below = area.bottom().min(buf.area.bottom());
    crate::anim::tab((area.x, area.y), active, Rect { x: area.x, y: below, width: area.width, height: buf.area.bottom() - below });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_close_next_prev_goto_and_reorder() {
        let mut t = Tabs::new("a");
        t.open("b");
        t.open("c"); // a b c, c active
        assert_eq!((t.len(), t.active_index(), *t.active()), (3, 2, "c"));
        t.next();
        assert_eq!(*t.active(), "a"); // wraps
        t.prev();
        assert_eq!(*t.active(), "c");
        assert!(t.goto(1) && *t.active() == "b" && !t.goto(9));
        // A new tab opens next to the active one.
        t.open("x");
        assert_eq!(t.iter().copied().collect::<Vec<_>>(), vec!["a", "b", "x", "c"]);
        assert_eq!(t.active_index(), 2);
        // Reorder: the active tab moves, staying in range.
        t.move_active(-1);
        assert_eq!(t.iter().copied().collect::<Vec<_>>(), vec!["a", "x", "b", "c"]);
        t.move_active(-5);
        assert_eq!((t.active_index(), *t.active()), (0, "x"));
        t.move_active(9);
        assert_eq!((t.active_index(), *t.active()), (3, "x"));
    }

    #[test]
    fn closing_remembers_for_reopen_and_the_last_tab_stays() {
        let mut t = Tabs::new(1);
        assert_eq!(t.close(), None, "the last tab cannot be closed");
        t.open(2);
        t.open(3);
        assert_eq!(t.close(), Some(())); // closes 3, 2 becomes active
        assert_eq!(*t.active(), 2);
        assert!(t.reopen());
        assert_eq!(t.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(*t.active(), 3);
        assert!(!t.reopen());
        t.duplicate();
        assert_eq!(t.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3, 3]);
        for i in 0..20 {
            t.open(100 + i);
            t.close();
        }
        // Only the last few closed tabs are remembered.
        let mut back = 0;
        while t.reopen() {
            back += 1;
        }
        assert_eq!(back, CLOSED_KEPT);
    }

    fn labels(names: &[&str]) -> Vec<Label> {
        names.iter().map(|n| Label::new(*n)).collect()
    }

    fn row(buf: &Buffer, width: u16) -> String {
        (0..width).map(|x| buf[(x, 0)].symbol().to_string()).collect::<String>().trim_end().to_string()
    }

    #[test]
    fn all_labels_show_numbered_when_they_fit() {
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        let mut ls = labels(&["README.md", "good.nix"]);
        ls[1].modified = true;
        render(area, &mut buf, &Theme::default(), &ls, 1, true);
        let text = row(&buf, 60);
        assert_eq!(text, " 1 README.md  2 good.nix ●");
        // The active label is bold, the other is not.
        let active = (0..60).find(|&x| buf[(x, 0)].symbol() == "g").unwrap();
        assert!(buf[(active, 0)].modifier.contains(Modifier::BOLD));
        let other = (0..60).find(|&x| buf[(x, 0)].symbol() == "R").unwrap();
        assert!(!buf[(other, 0)].modifier.contains(Modifier::BOLD) && !buf[(active, 0)].modifier.contains(Modifier::UNDERLINED));
        // Without numbers the labels are plain.
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &Theme::default(), &ls, 0, false);
        assert_eq!(row(&buf, 60), " README.md  good.nix ●");
    }

    #[test]
    fn a_crowded_strip_shows_a_window_around_the_active_tab_with_counts() {
        let names: Vec<String> = (0..9).map(|i| format!("file{i}.rs")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let ls = labels(&refs);
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &Theme::default(), &ls, 4, true);
        let text = row(&buf, 40);
        assert!(text.contains("5 file4.rs"), "the active tab is in view: {text:?}");
        assert!(text.starts_with('‹') && text.ends_with('›'), "both ends have counts: {text:?}");
        assert!(text.chars().count() <= 40, "{text:?}");
        // At the first tab nothing is hidden on the left.
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &Theme::default(), &ls, 0, true);
        let text = row(&buf, 40);
        assert!(text.contains("1 file0.rs") && !text.starts_with('‹') && text.ends_with('›'), "{text:?}");
        // One tab wider than the strip is cut with an ellipsis.
        let mut buf = Buffer::empty(Rect::new(0, 0, 12, 1));
        render(Rect::new(0, 0, 12, 1), &mut buf, &Theme::default(), &labels(&["a-very-long-file-name.rs"]), 0, true);
        let text = row(&buf, 12);
        assert!(text.contains('…') && text.chars().count() <= 12, "{text:?}");
    }

    #[test]
    fn equal_names_get_just_enough_parent_folders() {
        use std::path::Path;
        let t = |ps: &[&str]| short_titles(&ps.iter().map(|p| Path::new(*p)).collect::<Vec<_>>());
        assert_eq!(t(&["/a/src/lib.rs", "/a/ui/lib.rs", "/a/main.rs"]), vec!["src/lib.rs", "ui/lib.rs", "main.rs"]);
        // Deeper clashes need more levels, and only for the paths that clash.
        assert_eq!(t(&["/x/a/b/lib.rs", "/x/c/b/lib.rs", "/x/other.rs"]), vec!["a/b/lib.rs", "c/b/lib.rs", "other.rs"]);
        // Identical paths cannot be told apart: the whole path is shown for both.
        assert_eq!(t(&["/p/q.rs", "/p/q.rs"]), vec!["p/q.rs", "p/q.rs"]);
        assert_eq!(t(&["/"]), vec!["/"]);
        assert_eq!(t(&[]), Vec::<String>::new());
    }
}
