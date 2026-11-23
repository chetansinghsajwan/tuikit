//! Code blocks, inline code and math.

use super::state::Out;
use pulldown_cmark::CodeBlockKind;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ui::Theme;
use unicode_width::UnicodeWidthStr;

impl Out {
    /// Open a code block and note its language.
    pub(super) fn start_code_block(&mut self, kind: CodeBlockKind) {
        self.flush();
        self.in_code = true;
        self.code.clear();
        self.code_lang = match kind {
            CodeBlockKind::Fenced(l) => l.split([' ', ',', '{']).next().unwrap_or("").to_string(),
            CodeBlockKind::Indented => String::new(),
        };
    }

    /// Close a code block: a tinted, rounded block with the highlighted code.
    pub(super) fn end_code_block(&mut self, theme: &Theme) {
        let c = &theme.colors;
        let muted = Color::from(theme.colors.muted);
        self.in_code = false;
        let code = std::mem::take(&mut self.code);
        let lang = std::mem::take(&mut self.code_lang);
        let code = code.trim_end_matches('\n');
        // A tinted block with rounded corners and no frame. Its top and bottom rows are half blocks and
        // its corners quarter blocks, drawn in the tint colour on the terminal background, so the
        // corners are round and nothing is tinted outside them. The language is its first line.
        let total = self.width.saturating_sub(self.prefix_width() + 1).max(12);
        let content = total - 2;
        let tint = Style::new().bg(ui::mix(c.fg.into(), c.bg.into(), 7));
        let shade = Style::new().fg(tint.bg.unwrap_or(Color::Reset));
        self.push_line(vec![Span::styled(format!("▗{}▖", "▄".repeat(total - 2)), shade)]);
        // A margin of one column on both sides, the text padded self to the full width.
        let row = |spans: Vec<Span<'static>>, used: usize| -> Vec<Span<'static>> {
            let mut line = vec![Span::styled(" ", tint)];
            line.extend(spans);
            line.push(Span::styled(" ".repeat(total - 1 - used), tint));
            line
        };
        if !lang.is_empty() {
            let label: String = lang.chars().take(content).collect();
            let used = label.width();
            let spans = vec![Span::styled(label, tint.fg(muted).add_modifier(Modifier::ITALIC))];
            self.push_line(row(spans, used));
        }
        for pieces in crate::text::highlight_code(code, &lang, theme) {
            // Long lines are cut at the width.
            let mut spans = Vec::new();
            let mut used = 0;
            for (style, piece) in pieces {
                if used >= content {
                    break;
                }
                let piece: String = piece
                    .chars()
                    .scan(0usize, |w, ch| {
                        *w += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                        Some((*w, ch))
                    })
                    .take_while(|(w, _)| used + w <= content)
                    .map(|(_, ch)| ch)
                    .collect();
                used += piece.width();
                spans.push(Span::styled(piece, style.bg(tint.bg.unwrap_or(Color::Reset))));
            }
            self.push_line(row(spans, used));
        }
        if code.is_empty() {
            self.push_line(row(Vec::new(), 0));
        }
        self.push_line(vec![Span::styled(format!("▝{}▘", "▀".repeat(total - 2)), shade)]);

        self.lines.push(Line::default());
    }

    /// Inline code, a pill.
    pub(super) fn inline_code(&mut self, t: &str, theme: &Theme) {
        let c = &theme.colors;
        if let Some((_, alt)) = self.image.as_mut() {
            alt.push_str(&t);
        } else {
            let style = Style::new().fg(Color::from(c.syntax.string)).bg(Color::from(c.selection));
            // A pill: the text on its tint between rounded caps.
            self.words.push((ui::pill::word(&theme.icons, &t), style));
        }
    }

    /// Inline math.
    pub(super) fn inline_math(&mut self, t: &str, theme: &Theme) {
        let c = &theme.colors;
        self.words.push((
            t.to_string(),
            Style::new().fg(Color::from(c.info)).add_modifier(Modifier::ITALIC),
        ));
    }

    /// Display math, an indented block.
    pub(super) fn display_math(&mut self, t: &str, theme: &Theme) {
        let c = &theme.colors;
        self.flush();
        for l in t.trim().lines() {
            self.push_line(vec![Span::styled(
                format!("  {l}"),
                Style::new().fg(Color::from(c.info)).add_modifier(Modifier::ITALIC),
            )]);
        }
        self.lines.push(Line::default());
    }
}
