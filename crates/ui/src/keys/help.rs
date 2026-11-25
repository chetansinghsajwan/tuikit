use super::palette::Row;
use crate::component::Context;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Widget};

/// The `?` help: a scrollable popup listing every action with its keys. `/` starts a search
/// that filters the rows by name, keys or description.
#[derive(Debug, Default, Clone)]
pub struct Help {
    scroll: usize,
    query: String,
    searching: bool,
}

impl Help {
    /// Handle a key while the help is open; `false` closes it.
    pub fn key(&mut self, key: &KeyEvent) -> bool {
        if self.searching {
            match key.code {
                KeyCode::Esc => {
                    self.query.clear();
                    self.searching = false;
                }
                KeyCode::Enter => self.searching = false,
                KeyCode::Backspace => {
                    self.query.pop();
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.query.push(c)
                }
                KeyCode::Down => self.scroll += 1,
                KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
                _ => {}
            }
            return true;
        }
        match key.code {
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => self.scroll += 5,
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll = self.scroll.saturating_sub(5)
            }
            KeyCode::Down | KeyCode::Char('j') => self.scroll += 1,
            KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::PageDown => self.scroll += 10,
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(10),
            KeyCode::Home | KeyCode::Char('g') => self.scroll = 0,
            // Esc first clears a finished search, then closes.
            KeyCode::Esc if !self.query.is_empty() => self.query.clear(),
            KeyCode::Esc | KeyCode::Char('q' | '?') => return false,
            _ => {}
        }
        true
    }

    /// The rows that match the search (all of them with none).
    fn matching<'a>(&self, rows: &'a [Row]) -> Vec<&'a Row> {
        let q = self.query.to_lowercase();
        rows.iter()
            .filter(|r| {
                q.is_empty()
                    || r.name.to_lowercase().contains(&q)
                    || r.keys.to_lowercase().contains(&q)
                    || r.desc.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Draw the help popup over `area` with `rows` filtered by the search, dimming what is
    /// behind it.
    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        cx: &Context,
        title: &str,
        rows: &[Row],
    ) {
        let theme = cx.theme;
        let rect = crate::layout::popup(cx, area);
        if rect.width < 20 || rect.height < 6 {
            return;
        }
        cx.motion.overlay(buf, area);
        crate::dim::dim(buf, area, cx);
        let c = &theme.colors;
        Clear.render(rect, buf);
        let inner = crate::Pane::new().title(title).focused(true).draw(cx, rect, buf);
        // The search line and a rule under it, then the rows.
        let hint = if self.searching {
            String::new()
        } else if self.query.is_empty() {
            "/ to search".to_string()
        } else {
            String::new()
        };
        let line = if self.searching || !self.query.is_empty() {
            crate::prompt::line(cx, "/ ", &self.query)
        } else {
            Line::from(Span::styled(hint, Style::new().fg(c.muted.into())))
        };
        Paragraph::new(line).render(Rect { height: 1, ..inner }, buf);
        if self.searching {
            crate::prompt::cursor(
                buf,
                Rect { height: 1, ..inner },
                "/ ",
                self.query.chars().count(),
            );
        }
        buf.set_string(
            rect.x,
            inner.y + 1,
            format!(
                "{}{}{}",
                theme.icons.divider_left.glyph,
                theme.icons.divider.glyph.repeat(usize::from(rect.width).saturating_sub(2)),
                theme.icons.divider_right.glyph,
            ),
            Style::new().fg(c.border_focused.into()),
        );
        let list = Rect { y: inner.y + 2, height: inner.height.saturating_sub(2), ..inner };
        let shown = self.matching(rows);
        if shown.is_empty() {
            buf.set_string(list.x, list.y, "no match", Style::new().fg(c.muted.into()));
            return;
        }
        let keys_w = shown
            .iter()
            .map(|r| r.keys.chars().count())
            .max()
            .unwrap_or(0)
            .min(inner.width as usize / 2);
        let page = usize::from(list.height).max(1);
        self.scroll = self.scroll.min(shown.len().saturating_sub(page));
        for (i, r) in shown.iter().skip(self.scroll).take(page).enumerate() {
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<keys_w$}", r.keys),
                    Style::new().fg(c.warning.into()).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ", Style::new()),
                Span::styled(r.name.to_string(), Style::new().fg(c.accent.into())),
                Span::styled(format!("  {}", r.desc), Style::new().fg(c.muted.into())),
            ]);
            Paragraph::new(line).render(Rect { y: list.y + i as u16, height: 1, ..list }, buf);
        }
    }
}
