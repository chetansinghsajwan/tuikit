//! The one-line text prompt (`label value` with a block cursor), shared by
//! pick's query line and the explorer's new/rename/open-with prompts.

use crate::component::Context;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use tui_input::{Input, backend::crossterm::EventHandler};
use unicode_width::UnicodeWidthStr;

/// `label` in the accent color followed by the typed `value`.
pub fn line(cx: &Context, label: &str, value: &str) -> Line<'static> {
    let theme = cx.theme;
    let c = &theme.colors;
    Line::from(vec![
        Span::styled(label.to_string(), Style::new().fg(c.accent.into())),
        Span::styled(value.to_string(), Style::new().fg(c.fg.into())),
    ])
}

/// Mark the cursor cell: `cursor` columns into the value, after `label`, on the
/// row `area` (nothing if it falls outside).
pub fn cursor(buf: &mut Buffer, area: Rect, label: &str, cursor: usize) {
    let x = area.x + label.width() as u16 + cursor as u16;
    if x < area.right()
        && let Some(cell) = buf.cell_mut((x, area.y))
    {
        cell.set_style(Style::new().add_modifier(Modifier::REVERSED));
    }
}

/// What a key did to an open prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Typed {
    /// The text changed (or the key did nothing); the prompt stays open.
    Editing,
    /// Enter: the prompt is done, with its text.
    Submit(String),
    /// Esc: the prompt is closed without a result.
    Cancel,
}

/// Feeds a key event to the open prompt `input`: Esc cancels, Enter submits, anything else edits. The one
/// rule every prompt of every tool types by.
pub fn typed(input: &mut Input, ev: &Event) -> Typed {
    match crate::keys::pressed(ev).map(|k| k.code) {
        Some(KeyCode::Esc) => Typed::Cancel,
        Some(KeyCode::Enter) => Typed::Submit(input.value().to_string()),
        Some(_) => {
            input.handle_event(ev);
            Typed::Editing
        }
        None => Typed::Editing,
    }
}

/// Whether `code` answers a question with yes: `y`, `d` (the same key that asked) or Enter. Any other key
/// is a no.
pub fn confirms(code: KeyCode) -> bool {
    matches!(code, KeyCode::Char('y' | 'Y' | 'd' | 'D') | KeyCode::Enter)
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
    /// A suggestion list over `items`, with nothing typed and the first one selected.
    pub fn new(items: Vec<String>) -> Suggest {
        Suggest { items, typed: String::new(), selected: 0 }
    }

    /// What is typed changed: the selection starts over.
    pub fn set_typed_text(&mut self, typed: &str) {
        if self.typed != typed {
            self.typed = typed.to_string();
            self.selected = 0;
        }
    }

    /// The options containing what is typed (case-insensitive), the ones that start with it first.
    pub fn matches(&self) -> Vec<&str> {
        let q = self.typed.to_lowercase();
        let mut hits: Vec<&str> = self
            .items
            .iter()
            .map(String::as_str)
            .filter(|i| i.to_lowercase().contains(&q))
            .collect();
        hits.sort_by_key(|i| !i.to_lowercase().starts_with(&q));
        hits
    }

    /// Whether there are no options at all, so the tool can skip the suggestion popup.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Move the selection by `d` rows (wrapping); false when there is nothing to select.
    pub fn move_selection(&mut self, delta: isize) -> bool {
        let n = self.matches().len();
        if n == 0 {
            return false;
        }
        self.selected = (self.selected as isize + delta).rem_euclid(n as isize) as usize;
        true
    }

    /// The selected option, if anything matches.
    pub fn selected(&self) -> Option<String> {
        self.matches().get(self.selected).map(|s| s.to_string())
    }

    /// Draw the matches in a box above `prompt` (the row of the prompt line).
    pub fn render(&self, cx: &Context, prompt: Rect, buf: &mut Buffer) {
        let theme = cx.theme;
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
            let style = if on {
                crate::list::style(theme, true, None).fg(c.fg.into())
            } else {
                Style::new().bg(c.selection.into()).fg(c.fg.into())
            };
            buf.set_style(row, style);
            buf.set_stringn(row.x + 1, row.y, hit, usize::from(width.saturating_sub(2)), style);
        }
    }
}

#[cfg(test)]
mod tests;
