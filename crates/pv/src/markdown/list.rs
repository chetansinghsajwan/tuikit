//! Lists, their items and task markers.

use super::state::Out;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ui::Theme;
use unicode_width::UnicodeWidthStr;

impl Out {
    /// Open a list, numbered from `start` or bulleted.
    pub(super) fn start_list(&mut self, start: Option<u64>) {
        self.lists.push(start);
    }

    /// Open a list item with its marker.
    pub(super) fn start_item(&mut self, theme: &Theme) {
        let accent = Color::from(theme.colors.accent);
        self.flush();
        let depth = self.lists.len().saturating_sub(1);
        let marker = match self.lists.last_mut() {
            Some(Some(n)) => {
                let m = format!("{n}. ");
                *n += 1;
                m
            }
            _ => ["• ", "◦ ", "▪ "][depth % 3].to_string(),
        };
        let pad = " ".repeat(marker.width());
        self.bullet = Some(Span::styled(marker, Style::new().fg(accent)));
        self.indent.push(Span::raw(pad));
    }

    /// Close a list.
    pub(super) fn end_list(&mut self) {
        self.lists.pop();
        if self.lists.is_empty() {
            self.lines.push(Line::default());
        }
    }

    /// Close a list item.
    pub(super) fn end_item(&mut self) {
        self.flush();
        self.indent.pop();
        self.bullet = None;
    }

    /// The checkbox of a task list item.
    pub(super) fn task_marker(&mut self, done: bool, theme: &Theme) {
        let accent = Color::from(theme.colors.accent);
        self.bullet = Some(Span::styled(if done { "☑ " } else { "☐ " }, Style::new().fg(accent)));
    }
}
