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
mod tests;
