//! Columns: a list whose rows have fixed-width columns (a process list, a table of refs) and a header
//! over them. The tool says which columns there are and which one the list is sorted by; this decides
//! the widths, the alignment, the blank cell between columns, the sort marker, and drops the right
//! columns exactly when `ListRow` does, so the header and the rows can never drift apart.
//!
//! A row is the *lead* columns (before the name), the flexible name, then the *trail* columns at the
//! right edge. `lead_cells` and `trail_cells` pad a row's cells; the tool puts them in `ListRow::left`
//! and `ListRow::right` and passes `min_left`.

use crate::list;
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

/// Which side of its cell a column's text sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

/// One column: its header text, its width in cells and its alignment.
#[derive(Debug, Clone, Copy)]
pub struct Column {
    pub title: &'static str,
    pub width: usize,
    pub align: Align,
}

impl Column {
    /// A left-aligned column.
    pub fn left(title: &'static str, width: usize) -> Column {
        Column { title, width, align: Align::Left }
    }

    /// A right-aligned column (numbers, sizes).
    pub fn right(title: &'static str, width: usize) -> Column {
        Column { title, width, align: Align::Right }
    }
}

/// The columns of a list and what it is sorted by.
#[derive(Debug, Clone)]
pub struct Columns {
    lead: Vec<Column>,
    flex_title: &'static str,
    trail: Vec<Column>,
    min_left: usize,
    sorted: Option<(&'static str, bool)>,
}

impl Columns {
    /// Columns before the name (`lead`), the name's header (`flex_title`) and the columns at the right
    /// edge (`trail`). The trail is dropped when it would leave the left side under `min_left` cells.
    pub fn new(
        lead: Vec<Column>,
        flex_title: &'static str,
        trail: Vec<Column>,
        min_left: usize,
    ) -> Columns {
        Columns { lead, flex_title, trail, min_left, sorted: None }
    }

    /// Marks the column titled `title` as the sort column; `is_descending` points the marker down.
    pub fn sorted_by(mut self, title: &'static str, is_descending: bool) -> Columns {
        self.sorted = Some((title, is_descending));
        self
    }

    /// The room `ListRow::min_left` must keep for the left side, so rows and header drop the trail alike.
    pub fn min_left(&self) -> usize {
        self.min_left
    }

    /// The cells of a row's lead columns, padded to their widths, a blank cell after each. Put them
    /// first in `ListRow::left`. A missing cell is blank.
    pub fn lead_cells(&self, cells: Vec<Span<'static>>) -> Vec<Span<'static>> {
        Self::pad_all(&self.lead, cells, true)
    }

    /// The cells of a row's trail columns, padded to their widths, a blank cell between them. This is
    /// `ListRow::right`.
    pub fn trail_cells(&self, cells: Vec<Span<'static>>) -> Vec<Span<'static>> {
        Self::pad_all(&self.trail, cells, false)
    }

    /// The width of the trail: its columns and the blank cells between them.
    fn trail_width(&self) -> usize {
        self.trail.iter().map(|c| c.width).sum::<usize>() + self.trail.len().saturating_sub(1)
    }

    /// Whether the trail fits in `width` cells (the rule `ListRow` applies to its right column).
    fn shows_trail(&self, width: usize) -> bool {
        let trail = self.trail_width();
        trail > 0 && width.saturating_sub(trail + 1) >= self.min_left
    }

    /// `cells` fitted to `columns`; with `gap_after` each is followed by a blank cell, otherwise the
    /// blank cells go between them only.
    fn pad_all(
        columns: &[Column],
        cells: Vec<Span<'static>>,
        gap_after: bool,
    ) -> Vec<Span<'static>> {
        let mut cells = cells.into_iter();
        let mut out = Vec::new();
        for (i, column) in columns.iter().enumerate() {
            let cell = cells.next().unwrap_or_default();
            out.extend(Self::pad(cell, column));
            if gap_after || i + 1 < columns.len() {
                out.push(Span::raw(" "));
            }
        }
        out
    }

    /// One cell cut to the column's width (with `…`) and padded on the side the alignment leaves free.
    fn pad(cell: Span<'static>, column: &Column) -> Vec<Span<'static>> {
        let text = list::fit(&cell.content, column.width);
        let blank = Span::raw(" ".repeat(column.width.saturating_sub(text.width())));
        let cell = Span::styled(text, cell.style);
        match column.align {
            Align::Left => vec![cell, blank],
            Align::Right => vec![blank, cell],
        }
    }

    /// The header cell of a column: its title, accent and bold with a marker when it is the sort column.
    fn title_cell(&self, theme: &Theme, title: &'static str) -> Span<'static> {
        match self.sorted {
            Some((sorted, is_descending)) if sorted == title => Span::styled(
                format!("{title}{}", if is_descending { '▼' } else { '▲' }),
                Style::new().fg(theme.colors.accent.into()).add_modifier(Modifier::BOLD),
            ),
            _ => Span::styled(title, Style::new().fg(theme.colors.muted.into())),
        }
    }

    /// Draws the header on the one row `area` (the list's content width, the same `inner` the rows
    /// are drawn in).
    pub fn draw_header(&self, theme: &Theme, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let area = Rect { height: 1, ..area };
        let titles = self.lead.iter().map(|c| self.title_cell(theme, c.title)).collect();
        let mut left = self.lead_cells(titles);
        left.push(self.title_cell(theme, self.flex_title));
        let width = usize::from(area.width);
        let trail_width = if self.shows_trail(width) { self.trail_width() } else { 0 };
        let limit = width.saturating_sub(if trail_width > 0 { trail_width + 1 } else { 0 });
        let left = list::clip_spans(left, limit);
        Paragraph::new(Line::from(left)).render(Rect { width: limit as u16, ..area }, buf);
        if trail_width > 0 {
            let titles = self.trail.iter().map(|c| self.title_cell(theme, c.title)).collect();
            let right = self.trail_cells(titles);
            let x = area.right() - trail_width as u16;
            Paragraph::new(Line::from(right))
                .render(Rect { x, width: trail_width as u16, ..area }, buf);
        }
    }
}

/// Tests for the columns.
#[cfg(test)]
mod tests;
