//! Front matter.

use super::state::Out;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

impl Out {
    /// Open the front matter.
    pub(super) fn start_front_matter(&mut self) {
        self.meta = Some(String::new());
    }

    /// Close the front matter: draw it between rules.
    pub(super) fn end_front_matter(&mut self, cx: &ui::Context) {
        let theme = cx.theme;
        let muted = Color::from(theme.colors.muted);
        if let Some(meta) = self.meta.take() {
            let rule = Span::styled(
                "┈".repeat(self.width.saturating_sub(self.prefix_width()).min(60)),
                Style::new().fg(muted),
            );
            self.push_line(vec![rule.clone()]);
            for l in meta.trim_end().lines() {
                self.push_line(vec![Span::styled(l.to_string(), Style::new().fg(muted))]);
            }
            self.push_line(vec![rule]);
            self.lines.push(Line::default());
        }
    }
}
