//! A popup to choose several things at once, in sections: pick any number of items of a list,
//! one item, or type a text. What is chosen shows in the popup, in every section, as you go.
//!
//! Keys: Tab / Shift-Tab (or Left / Right) switch the section, Up / Down (Ctrl-N / Ctrl-P) move,
//! Space chooses or drops the item under the cursor, typing narrows the list (or is the text of a
//! text section), Backspace erases, Ctrl-X clears the section, Enter applies, Esc cancels.

use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Widget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Any number of the items.
    Many,
    /// At most one of the items.
    One,
    /// A line of text.
    Text,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub title: String,
    pub kind: Kind,
    pub items: Vec<String>,
    /// What is chosen (for `Text`: the text, if any).
    pub chosen: Vec<String>,
}

impl Section {
    pub fn new(title: &str, kind: Kind, items: Vec<String>, chosen: Vec<String>) -> Section {
        Section { title: title.to_string(), kind, items, chosen }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Open,
    Apply,
    Cancel,
}

#[derive(Debug)]
pub struct Multi {
    title: String,
    pub sections: Vec<Section>,
    current: usize,
    cursor: usize,
    /// Narrows the items of the current section.
    query: String,
}

impl Multi {
    pub fn new(title: &str, sections: Vec<Section>) -> Multi {
        Multi { title: title.to_string(), sections, current: 0, cursor: 0, query: String::new() }
    }

    /// The items of the current section that match what was typed.
    fn visible(&self) -> Vec<&str> {
        let q = self.query.to_lowercase();
        self.sections[self.current].items.iter().map(String::as_str).filter(|i| i.to_lowercase().contains(&q)).collect()
    }

    fn switch(&mut self, d: isize) {
        self.current = (self.current as isize + d).rem_euclid(self.sections.len() as isize) as usize;
        self.cursor = 0;
        self.query.clear();
    }

    pub fn key(&mut self, key: &KeyEvent) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let kind = self.sections[self.current].kind;
        match key.code {
            KeyCode::Esc => return Outcome::Cancel,
            KeyCode::Enter => return Outcome::Apply,
            KeyCode::Tab | KeyCode::Right => self.switch(1),
            KeyCode::BackTab | KeyCode::Left => self.switch(-1),
            KeyCode::Down => self.cursor = (self.cursor + 1).min(self.visible().len().saturating_sub(1)),
            KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Char('n') if ctrl => self.cursor = (self.cursor + 1).min(self.visible().len().saturating_sub(1)),
            KeyCode::Char('p') if ctrl => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Char('x') if ctrl => {
                self.sections[self.current].chosen.clear();
                self.query.clear();
            }
            KeyCode::Char(' ') if kind != Kind::Text => {
                if let Some(item) = self.visible().get(self.cursor).map(|s| s.to_string()) {
                    let s = &mut self.sections[self.current];
                    match s.chosen.iter().position(|c| *c == item) {
                        Some(at) => {
                            s.chosen.remove(at);
                        }
                        None => {
                            if kind == Kind::One {
                                s.chosen.clear();
                            }
                            s.chosen.push(item);
                        }
                    }
                }
            }
            KeyCode::Char(c) if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => {
                if kind == Kind::Text {
                    let s = &mut self.sections[self.current];
                    match s.chosen.first_mut() {
                        Some(t) => t.push(c),
                        None => s.chosen.push(c.to_string()),
                    }
                } else {
                    self.query.push(c);
                    self.cursor = 0;
                }
            }
            KeyCode::Backspace => {
                if kind == Kind::Text {
                    let s = &mut self.sections[self.current];
                    if let Some(t) = s.chosen.first_mut() {
                        t.pop();
                        if t.is_empty() {
                            s.chosen.clear();
                        }
                    }
                } else {
                    self.query.pop();
                    self.cursor = 0;
                }
            }
            _ => {}
        }
        Outcome::Open
    }

    /// The chosen items of the section called `title`.
    pub fn chosen(&self, title: &str) -> &[String] {
        self.sections.iter().find(|s| s.title == title).map(|s| s.chosen.as_slice()).unwrap_or_default()
    }

    /// Draw the popup over `area`, dimming what is behind it.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width < 30 || area.height < 10 {
            return;
        }
        crate::dim::dim(buf, area, theme);
        let c = &theme.colors;
        let (width, height) = (area.width.min(68), area.height.saturating_sub(2).min(24));
        let rect = Rect { x: area.x + (area.width - width) / 2, y: area.y + (area.height - height) / 2, width, height };
        Clear.render(rect, buf);
        let pane = crate::Pane::new(theme).title(&self.title).focused(true);
        let inner = pane.inner(rect);
        pane.render(rect, buf);
        let (fg, muted, accent) =
            (Style::new().fg(c.fg.into()), Style::new().fg(c.muted.into()), Style::new().fg(c.accent.into()));
        let row = |n: u16| Rect { y: inner.y + n, height: 1, ..inner };
        // The sections as tabs, with how many things each has chosen.
        let mut tabs = Vec::new();
        for (i, s) in self.sections.iter().enumerate() {
            let label = if s.chosen.is_empty() { s.title.clone() } else { format!("{} ({})", s.title, s.chosen.len()) };
            let style = if i == self.current {
                accent.add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else if s.chosen.is_empty() {
                muted
            } else {
                fg
            };
            tabs.push(Span::styled(label, style));
            tabs.push(Span::styled("  ", muted));
        }
        Paragraph::new(Line::from(tabs)).render(row(0), buf);
        // The current section: the text, or the items with check marks.
        let s = &self.sections[self.current];
        let list_h = usize::from(inner.height).saturating_sub(5);
        if s.kind == Kind::Text {
            let text = s.chosen.first().cloned().unwrap_or_default();
            Paragraph::new(Line::from(vec![
                Span::styled("contains: ", accent),
                Span::styled(text, fg),
                Span::styled("▏", accent),
            ]))
            .render(row(2), buf);
        } else {
            let query = if self.query.is_empty() {
                Span::styled("type to narrow the list", muted)
            } else {
                Span::styled(self.query.clone(), fg)
            };
            Paragraph::new(Line::from(vec![Span::styled("> ", accent), query])).render(row(2), buf);
            let items = self.visible();
            let first = self.cursor.saturating_sub(list_h.saturating_sub(1)).min(items.len().saturating_sub(list_h));
            for (n, item) in items.iter().skip(first).take(list_h).enumerate() {
                let on = s.chosen.iter().any(|c| c == item);
                let at = first + n == self.cursor;
                let mark = match (s.kind, on) {
                    (Kind::One, true) => "(•) ",
                    (Kind::One, false) => "( ) ",
                    (_, true) => "[x] ",
                    (_, false) => "[ ] ",
                };
                let line = Line::from(vec![
                    Span::styled(mark, if on { Style::new().fg(c.success.into()) } else { muted }),
                    Span::styled(item.to_string(), fg),
                ]);
                let r = row(3 + n as u16);
                if at {
                    buf.set_style(r, crate::list::style(theme, true, None));
                }
                Paragraph::new(line).render(r, buf);
            }
            if items.is_empty() {
                Paragraph::new(Line::from(Span::styled("nothing matches", muted))).render(row(3), buf);
            }
        }
        // What is chosen so far, everywhere, and the keys.
        let all: Vec<String> = self
            .sections
            .iter()
            .filter(|s| !s.chosen.is_empty())
            .map(|s| format!("{}: {}", s.title.to_lowercase(), s.chosen.join(", ")))
            .collect();
        let summary = if all.is_empty() { "nothing chosen".to_string() } else { all.join("  ·  ") };
        Paragraph::new(Line::from(vec![
            Span::styled("chosen  ", muted),
            Span::styled(summary, Style::new().fg(c.warning.into())),
        ]))
        .render(row(inner.height.saturating_sub(2)), buf);
        Paragraph::new(Line::from(Span::styled(
            "space choose  tab section  enter apply  ctrl-x clear  esc cancel",
            muted,
        )))
        .render(row(inner.height.saturating_sub(1)), buf);
    }
}

#[cfg(test)]
mod tests;
