//! Block quotes and alerts.

use super::state::Out;
use pulldown_cmark::BlockQuoteKind;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ui::Theme;

/// The color and name of an alert (`> [!NOTE]`).
fn alert(kind: BlockQuoteKind, theme: &Theme) -> (Color, &'static str) {
    let c = &theme.colors;
    match kind {
        BlockQuoteKind::Note => (c.info.into(), "ⓘ Note"),
        BlockQuoteKind::Tip => (c.success.into(), "💡 Tip"),
        BlockQuoteKind::Important => (c.accent.into(), "❗ Important"),
        BlockQuoteKind::Warning => (c.warning.into(), "⚠ Warning"),
        BlockQuoteKind::Caution => (c.error.into(), "⛔ Caution"),
    }
}

impl Out {
    /// Open a block quote, or an alert (`> [!NOTE]`) with its title.
    pub(super) fn start_quote(&mut self, kind: Option<BlockQuoteKind>, theme: &Theme) {
        let muted = Color::from(theme.colors.muted);
        self.flush();
        match kind {
            Some(k) => {
                let (color, title) = alert(k, theme);
                self.indent.push(Span::styled("▏ ", Style::new().fg(color)));
                self.push_line(vec![Span::styled(
                    title,
                    Style::new().fg(color).add_modifier(Modifier::BOLD),
                )]);
            }
            None => self.indent.push(Span::styled("▏ ", Style::new().fg(muted))),
        }
    }

    /// Close a block quote.
    pub(super) fn end_quote(&mut self) {
        self.flush();
        self.indent.pop();
    }
}
