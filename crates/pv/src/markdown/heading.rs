//! Headings.

use super::state::Out;
use pulldown_cmark::HeadingLevel;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

impl Out {
    /// Open a heading: its marker, and the bold accent style.
    pub(super) fn start_heading(&mut self, level: HeadingLevel, cx: &ui::Context) {
        let theme = cx.theme;
        let muted = Color::from(theme.colors.muted);
        let accent = Color::from(theme.colors.accent);
        self.blank();
        self.headings.push(self.lines.len());
        let mark = match level {
            HeadingLevel::H1 => "█ ",
            HeadingLevel::H2 => "▌ ",
            _ => "▎ ",
        };
        self.words.push((mark.trim_end().to_string(), Style::new().fg(muted)));
        self.words.push((String::new(), Style::new()));
        self.styles.push(Style::new().fg(accent).add_modifier(Modifier::BOLD));
    }

    /// Close a heading: big text for a level one that fits, else the marked line.
    pub(super) fn end_heading(&mut self, level: HeadingLevel) {
        // A level one heading is drawn big when it fits, else as the marked line.
        if level == HeadingLevel::H1
            && let Some(rows) = self.big_heading()
        {
            let style = self.style();
            self.words.clear();
            for row in rows {
                self.push_line(vec![Span::styled(row, style)]);
            }
        }
        self.flush();
        self.styles.pop();
        self.lines.push(Line::default());
    }

    /// The rows of the heading being collected as big text, when all of it has glyphs and it fits.
    pub(super) fn big_heading(&self) -> Option<[String; ui::bigtext::HEIGHT]> {
        // The first two words are the marker and its separator.
        let text: Vec<&str> =
            self.words.iter().skip(2).map(|(w, _)| w.as_str()).filter(|w| !w.is_empty()).collect();
        let rows = ui::bigtext::render(&text.join(" "))?;
        let avail = self.width.saturating_sub(self.prefix_width() + 2).max(10);
        (rows[0].chars().count() <= avail).then_some(rows)
    }
}
