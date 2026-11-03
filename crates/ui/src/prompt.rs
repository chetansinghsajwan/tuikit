//! The one-line text prompt (`label value` with a block cursor), shared by
//! pick's query line and the explorer's new/rename/open-with prompts.

use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// `label` in the accent color followed by the typed `value`.
pub fn line(theme: &Theme, label: &str, value: &str) -> Line<'static> {
    let c = &theme.colors;
    Line::from(vec![Span::styled(label.to_string(), Style::new().fg(c.accent.into())), Span::styled(value.to_string(), Style::new().fg(c.fg.into()))])
}

/// Mark the cursor cell: `cursor` columns into the value, after `label`, on the
/// row `area` (nothing if it falls outside).
pub fn cursor(buf: &mut Buffer, area: Rect, label: &str, cursor: usize) {
    let x = area.x + label.width() as u16 + cursor as u16;
    if x < area.right() {
        if let Some(cell) = buf.cell_mut((x, area.y)) {
            cell.set_style(Style::new().add_modifier(Modifier::REVERSED));
        }
    }
}

/// The suggestions for a prompt: the options, the ones matching what is typed, and the selected one.
#[derive(Debug, Default, Clone)]
pub struct Suggest {
    items: Vec<String>,
    typed: String,
    selected: usize,
}

/// How many suggestions show at once.
const SUGGEST_ROWS: usize = 8;

impl Suggest {
    pub fn new(items: Vec<String>) -> Suggest {
        Suggest { items, typed: String::new(), selected: 0 }
    }

    /// What is typed changed: the selection starts over.
    pub fn typed(&mut self, typed: &str) {
        if self.typed != typed {
            self.typed = typed.to_string();
            self.selected = 0;
        }
    }

    /// The options containing what is typed (case-insensitive), the ones that start with it first.
    pub fn matches(&self) -> Vec<&str> {
        let q = self.typed.to_lowercase();
        let mut hits: Vec<&str> = self.items.iter().map(String::as_str).filter(|i| i.to_lowercase().contains(&q)).collect();
        hits.sort_by_key(|i| !i.to_lowercase().starts_with(&q));
        hits
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Move the selection by `d` rows (wrapping); false when there is nothing to select.
    pub fn step(&mut self, d: isize) -> bool {
        let n = self.matches().len();
        if n == 0 {
            return false;
        }
        self.selected = (self.selected as isize + d).rem_euclid(n as isize) as usize;
        true
    }

    /// The selected option, if anything matches.
    pub fn current(&self) -> Option<String> {
        self.matches().get(self.selected).map(|s| s.to_string())
    }

    /// Draw the matches in a box above `prompt` (the row of the prompt line).
    pub fn render(&self, buf: &mut Buffer, prompt: Rect, theme: &Theme) {
        let hits = self.matches();
        let rows = hits.len().min(SUGGEST_ROWS).min(usize::from(prompt.y.saturating_sub(1)));
        if rows == 0 {
            return;
        }
        let c = &theme.colors;
        let wide = hits.iter().map(|h| h.width()).max().unwrap_or(0) as u16 + 4;
        let width = wide.min(prompt.width).max(10.min(prompt.width));
        // The window around the selection.
        let first = self.selected.saturating_sub(rows - 1).min(hits.len() - rows);
        for (i, hit) in hits.iter().skip(first).take(rows).enumerate() {
            let row = Rect { x: prompt.x, y: prompt.y - (rows - i) as u16, width, height: 1 };
            let on = first + i == self.selected;
            let style = if on { crate::list::style(theme, true, None).fg(c.fg.into()) } else { Style::new().bg(c.selection.into()).fg(c.fg.into()) };
            buf.set_style(row, style);
            buf.set_stringn(row.x + 1, row.y, hit, usize::from(width.saturating_sub(2)), style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_cell_is_reversed_after_the_label() {
        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);
        cursor(&mut buf, area, "> ", 3);
        assert!(buf[(5, 0)].modifier.contains(Modifier::REVERSED));
        assert!(!buf[(4, 0)].modifier.contains(Modifier::REVERSED));
        cursor(&mut buf, area, "> ", 20); // outside: no panic, no mark
    }

    #[test]
    fn suggestions_match_anywhere_prefixes_first_and_step_around() {
        let mut s = Suggest::new(vec!["Ada Lovelace".into(), "Grace Hopper".into(), "Alan Adams".into()]);
        s.typed("ad");
        assert_eq!(s.matches(), ["Ada Lovelace", "Alan Adams"]);
        assert_eq!(s.current().as_deref(), Some("Ada Lovelace"));
        assert!(s.step(1));
        assert_eq!(s.current().as_deref(), Some("Alan Adams"));
        assert!(s.step(1));
        assert_eq!(s.current().as_deref(), Some("Ada Lovelace"));
        s.typed("zzz");
        assert!(!s.step(1) && s.current().is_none());
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 6));
        s.typed("");
        s.render(&mut buf, Rect::new(0, 5, 30, 1), &Theme::default());
        assert_eq!(buf[(1, 4)].symbol(), "A");
    }

    #[test]
    fn line_has_the_label_then_the_value() {
        let l = line(&Theme::default(), "new: ", "a.rs");
        assert_eq!(l.spans.iter().map(|s| s.content.as_ref()).collect::<String>(), "new: a.rs");
    }
}
