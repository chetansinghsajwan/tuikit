//! The footer: the one line at the bottom of a pane or a screen. It shows a prompt being typed, a
//! question being asked, a job that runs, the last message, or the key hints, so every tool says
//! these things in the same colors and the same place.

use crate::component::{Component, Context};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

/// The last thing a tool said: a short text, shown until the next key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub is_error: bool,
}

impl Notice {
    /// A message, in the error color when `is_error`.
    pub fn new(text: impl Into<String>, is_error: bool) -> Notice {
        Notice { text: text.into(), is_error }
    }
}

/// What the footer line shows.
pub enum Footer<'a> {
    /// A prompt being typed: `label` in the accent color, then the `text` shown (masked for a
    /// password) with the cursor `cursor` cells into it.
    Prompt { label: &'a str, text: String, cursor: usize },
    /// A question that waits for a key (`Delete x? d again`).
    Question(String),
    /// The last message.
    Notice(&'a Notice),
    /// A job that runs, with a spinner.
    Busy(&'a str),
    /// The key hints or other quiet information.
    Hint(String),
}

impl Footer<'_> {
    /// The line to draw.
    pub fn line(&self, cx: &Context) -> Line<'static> {
        let c = &cx.theme.colors;
        match self {
            Footer::Prompt { label, text, .. } => crate::prompt::line(cx, label, text),
            Footer::Question(q) => {
                Line::from(Span::styled(q.clone(), Style::new().fg(c.warning.into())))
            }
            Footer::Notice(n) => Line::from(Span::styled(
                n.text.clone(),
                Style::new().fg(if n.is_error { c.error } else { c.muted }.into()),
            )),
            Footer::Busy(label) => Line::from(Span::styled(
                format!("{} {label}", cx.motion.spinner()),
                Style::new().fg(c.accent.into()),
            )),
            Footer::Hint(h) => Line::from(Span::styled(h.clone(), Style::new().fg(c.muted.into()))),
        }
    }
}

/// Drawn on the row `area`, with the cursor for a prompt.
impl Component for Footer<'_> {
    fn render(self, cx: &Context, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.line(cx)).render(area, buf);
        if let Footer::Prompt { label, cursor, .. } = self {
            crate::prompt::cursor(buf, area, label, cursor);
        }
    }
}

/// `text` as the dots of a password prompt.
pub fn mask(text: &str) -> String {
    "•".repeat(text.chars().count())
}

#[cfg(test)]
mod tests;
