//! The state a tool keeps for a table between frames: the cursor, the scroll, the sort and the widths
//! measured so far. The rows and the columns are not kept: they are passed to `view` each frame.

use super::layout::{Column, Overflow};
use super::view::TableView;
use super::{Row, SCROLL_OFF};
use crate::app::Navigable;
use crate::list::move_cursor;

/// What the table is sorted by: a column's id and the direction. The tool reads it and sorts its data;
/// the header shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sort {
    pub column: &'static str,
    pub descending: bool,
}

/// A table a tool keeps: where the cursor is (a row, and a column in cell mode), how far it is
/// scrolled, which rows the cursor can land on (all of them, unless the tool says otherwise with
/// `set_rows`), the sort, and the widths its `Fit` columns grew to. It is `Navigable`, so the shell's
/// movement actions move it. It is drawn with `view`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub(crate) cursor: usize,
    pub(crate) first_visible_row: usize,
    /// How many rows stay visible between the cursor and the edge while scrolling (vim's `scrolloff`).
    pub(crate) scroll_off: usize,
    /// The rows the last frame showed, for page moves.
    pub(crate) page: usize,
    pub(crate) len: usize,
    /// The rows the cursor can land on, ascending; `None` when every row can take it.
    selectable: Option<Vec<usize>>,
    /// The rows the last frame drew, top to bottom (pinned rows first).
    pub(crate) shown: Vec<usize>,
    pub(crate) sort: Option<Sort>,
    /// The widths `Width::Fit` columns grew to, by column id and place.
    pub(crate) fit: Vec<(&'static str, usize, u16)>,
    /// The cursor's column in cell mode (`None`: the cursor is a whole row).
    pub(crate) col: Option<usize>,
    /// How many columns the last frame had, for column moves.
    pub(crate) cols: usize,
    /// The first column drawn when the table scrolls sideways.
    pub(crate) first_col: usize,
    pub(crate) overflow: Overflow,
}

impl Default for Table {
    fn default() -> Self {
        Table {
            cursor: 0,
            first_visible_row: 0,
            scroll_off: SCROLL_OFF,
            page: 1,
            len: 0,
            selectable: None,
            shown: Vec::new(),
            sort: None,
            fit: Vec::new(),
            col: None,
            cols: 0,
            first_col: 0,
            overflow: Overflow::Drop,
        }
    }
}

impl Table {
    /// An empty table with the cursor on the first row and the default `scroll_off`.
    pub fn new() -> Table {
        Table::default()
    }

    /// Keep `rows` rows between the cursor and the edge while scrolling, instead of `SCROLL_OFF`.
    pub fn with_scroll_off(mut self, rows: usize) -> Table {
        self.scroll_off = rows;
        self
    }

    /// The cursor is a cell, not a row (a spreadsheet): it has a column too, and the table scrolls
    /// sideways to keep it in view instead of dropping columns.
    pub fn with_cells(mut self) -> Table {
        self.col = Some(0);
        self.overflow = Overflow::Scroll;
        self
    }

    /// What happens when the columns do not fit.
    pub fn with_overflow(mut self, overflow: Overflow) -> Table {
        self.overflow = overflow;
        self
    }

    /// Sorted by the column `column`, in this direction, from the start.
    pub fn with_sort(mut self, column: &'static str, descending: bool) -> Table {
        self.sort = Some(Sort { column, descending });
        self
    }

    /// The row the cursor is on.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The column the cursor is on in cell mode (0 otherwise).
    pub fn col(&self) -> usize {
        self.col.unwrap_or(0)
    }

    /// The first row that scrolls (below any pinned rows).
    pub fn first_visible_row(&self) -> usize {
        self.first_visible_row
    }

    /// The first column drawn when the table scrolls sideways.
    pub fn first_col(&self) -> usize {
        self.first_col
    }

    /// How many rows the table has, as last told by `set_len`, `set_rows` or `view`.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the table has no rows.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The rows the last frame drew, top to bottom with the pinned rows first: what is on screen, for
    /// work that only matters there (counting the sizes of the folders shown).
    pub fn shown(&self) -> &[usize] {
        &self.shown
    }

    /// The table now has `len` rows, all of which the cursor can land on; the cursor stays inside it.
    pub fn set_len(&mut self, len: usize) {
        self.len = len;
        self.selectable = None;
        self.clamp();
    }

    /// The table now has `len` rows, of which the cursor lands only on those `can_select` accepts (not
    /// on group headers); the cursor moves to the nearest such row.
    pub fn set_rows(&mut self, len: usize, can_select: impl Fn(usize) -> bool) {
        self.len = len;
        self.selectable = Some((0..len).filter(|&i| can_select(i)).collect());
        self.clamp();
    }

    /// Puts the cursor on row `row` (or the nearest row it can land on).
    pub fn set_cursor(&mut self, row: usize) {
        self.cursor = row;
        self.clamp();
    }

    /// The table now has `cols` columns, for column moves before the next frame tells it.
    pub fn set_cols(&mut self, cols: usize) {
        self.cols = cols;
        if let Some(col) = self.col {
            self.col = Some(col.min(cols.saturating_sub(1)));
        }
    }

    /// Puts the cursor on column `col` in cell mode.
    pub fn set_col(&mut self, col: usize) {
        if self.col.is_some() {
            self.col = Some(col.min(self.cols.saturating_sub(1)));
        }
    }

    /// Back to the top left: the cursor on the first row, nothing scrolled.
    pub fn reset(&mut self) {
        self.first_visible_row = 0;
        self.cursor = 0;
        self.first_col = 0;
        if self.col.is_some() {
            self.col = Some(0);
        }
        self.clamp();
    }

    /// Forgets the widths the `Fit` columns grew to: for new data (another file, another folder).
    pub fn reset_widths(&mut self) {
        self.fit.clear();
    }

    /// What the table is sorted by.
    pub fn sort(&self) -> Option<Sort> {
        self.sort
    }

    /// Sort by the column `column`, in this direction.
    pub fn set_sort(&mut self, column: &'static str, descending: bool) {
        self.sort = Some(Sort { column, descending });
    }

    /// Sort by the next of `columns` that can sort, in the direction it starts in, after the last one.
    pub fn sort_next(&mut self, columns: &[Column]) {
        let sortable: Vec<&Column> = columns.iter().filter(|c| c.sort.is_some()).collect();
        if sortable.is_empty() {
            return;
        }
        let at = self.sort.and_then(|s| sortable.iter().position(|c| c.id == s.column));
        let next = sortable[at.map_or(0, |i| (i + 1) % sortable.len())];
        self.sort = Some(Sort { column: next.id, descending: next.sort == Some(true) });
    }

    /// Turn the sort around.
    pub fn reverse_sort(&mut self) {
        if let Some(s) = &mut self.sort {
            s.descending = !s.descending;
        }
    }

    /// Whether the sort runs against the direction its column starts in (`by cpu reversed`).
    pub fn is_sort_reversed(&self, columns: &[Column]) -> bool {
        self.sort.is_some_and(|s| {
            columns.iter().find(|c| c.id == s.column).is_some_and(|c| c.sort != Some(s.descending))
        })
    }

    /// The rows the cursor can land on, in order.
    fn stops(&self) -> Vec<usize> {
        self.selectable.clone().unwrap_or_else(|| (0..self.len).collect())
    }

    /// Keeps the cursor inside the table and on a row it can land on, the nearest one at or above it first.
    pub(crate) fn clamp(&mut self) {
        match &self.selectable {
            None => self.cursor = self.cursor.min(self.len.saturating_sub(1)),
            Some(stops) => {
                let at = stops.partition_point(|&r| r <= self.cursor);
                self.cursor = match (at.checked_sub(1).map(|i| stops[i]), stops.get(at)) {
                    (Some(r), _) | (None, Some(&r)) => r,
                    (None, None) => 0,
                };
            }
        }
    }

    /// Draws the table's `len` rows in `columns`, asking `row` for each visible one by its index. The
    /// cursor, the focus and the layout are the table's.
    pub fn view<'l, F: FnMut(usize) -> Row>(
        &'l mut self,
        len: usize,
        columns: &'l [Column],
        row: F,
    ) -> TableView<'l, F> {
        if len != self.len {
            self.set_len(len);
        }
        TableView::new(self, columns, row)
    }

    /// Moves the cursor by `delta` of the rows it can land on, stopping at the first and the last.
    pub fn move_by(&mut self, delta: isize) {
        let stops = self.stops();
        let at = stops.iter().position(|&r| r == self.cursor).unwrap_or(0);
        if let Some(&r) = stops.get(move_cursor(at, delta, stops.len())) {
            self.cursor = r;
        }
    }

    /// Moves the cursor by `delta` columns in cell mode, stopping at the first and the last.
    pub fn move_col_by(&mut self, delta: isize) {
        if let Some(col) = self.col {
            self.col = Some(move_cursor(col, delta, self.cols));
        }
    }

    /// How many rows the last frame showed: what a page move covers.
    pub fn page_row_count(&self) -> usize {
        self.page
    }

    /// Moves the cursor to the first row it can land on.
    pub fn move_to_top(&mut self) {
        self.cursor = self.stops().first().copied().unwrap_or(0);
    }

    /// Moves the cursor to the last row it can land on.
    pub fn move_to_bottom(&mut self) {
        self.cursor = self.stops().last().copied().unwrap_or(0);
    }
}

impl Navigable for Table {
    fn move_by(&mut self, delta: isize) {
        Table::move_by(self, delta);
    }

    fn page_row_count(&self) -> usize {
        Table::page_row_count(self)
    }

    fn move_to_top(&mut self) {
        Table::move_to_top(self);
    }

    fn move_to_bottom(&mut self) {
        Table::move_to_bottom(self);
    }
}
