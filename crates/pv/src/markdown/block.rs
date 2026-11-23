//! Paragraphs and rules.

use super::state::Out;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ui::Theme;

impl Out {
    /// Close a paragraph.
    pub(super) fn end_paragraph(&mut self) {
        self.flush();
        if self.lists.is_empty() || self.indent.is_empty() {
            self.lines.push(Line::default());
        }
    }

    /// A horizontal rule.
    pub(super) fn rule(&mut self, theme: &Theme) {
        let muted = Color::from(theme.colors.muted);
        self.flush();
        let w = self.width.saturating_sub(self.prefix_width()).min(60);
        self.lines.push(Line::from(Span::styled("─".repeat(w), Style::new().fg(muted))));
        self.lines.push(Line::default());
    }
}
