//! Footnote definitions and references.

use super::state::Out;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ui::Theme;
use unicode_width::UnicodeWidthStr;

impl Out {
    /// Open a footnote definition with its number.
    pub(super) fn start_footnote(&mut self, name: &str, theme: &Theme) {
        let muted = Color::from(theme.colors.muted);
        self.flush();
        self.blank();
        let n = self.note(name);
        let marker = format!("[{n}] ");
        let pad = " ".repeat(marker.width());
        self.bullet = Some(Span::styled(marker, Style::new().fg(muted)));
        self.indent.push(Span::raw(pad));
    }

    /// Close a footnote definition.
    pub(super) fn end_footnote(&mut self) {
        self.flush();
        self.indent.pop();
        self.bullet = None;
    }

    /// A reference to a footnote, as its number.
    pub(super) fn footnote_reference(&mut self, name: &str, theme: &Theme) {
        let accent = Color::from(theme.colors.accent);
        let n = self.note(name);
        match self.words.last_mut() {
            Some(last) if !last.0.is_empty() => last.0.push_str(&format!("[{n}]")),
            _ => self.words.push((format!("[{n}]"), Style::new().fg(accent))),
        }
    }
}
