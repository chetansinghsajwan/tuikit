//! One frame of a table: it scrolls to keep the cursor in view, asks for the visible rows, lays the
//! columns out once for all of them, and draws the header, the tree guides, the rows and the cursor.

use super::layout::{Column, GAP, Kind, Overflow, Width, resolve};
use super::row::{self, Cell, Row, Rows, State, title_width};
use super::scroll::{follow_cursor, scroll_margin};
use super::state::Table;
use crate::component::{Component, Context};
use crate::tree;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

/// Rows pinned above the scrolling ones, given the first row that scrolls.
type Pinned<'l> = Box<dyn Fn(usize) -> Vec<usize> + 'l>;
/// The depth of a row of a tree.
type Depth<'l> = Box<dyn Fn(usize) -> usize + 'l>;

/// One frame of a `Table`, built by `Table::view` and drawn into the area it is given.
pub struct TableView<'l, F> {
    table: &'l mut Table,
    columns: &'l [Column],
    row: F,
    pane: Option<Rect>,
    focused: bool,
    cursor: bool,
    header: bool,
    empty: Option<&'l str>,
    pinned: Option<Pinned<'l>>,
    depth: Option<Depth<'l>>,
}

impl<'l, F: FnMut(usize) -> Row> TableView<'l, F> {
    pub(crate) fn new(table: &'l mut Table, columns: &'l [Column], row: F) -> Self {
        TableView {
            table,
            columns,
            row,
            pane: None,
            focused: false,
            cursor: true,
            header: false,
            empty: None,
            pinned: None,
            depth: None,
        }
    }

    /// The whole pane the table sits in, so the row bars reach into its padding.
    pub fn pane(mut self, pane: Rect) -> Self {
        self.pane = Some(pane);
        self
    }

    /// Whether the table's pane has the focus: the cursor row is then the bright one.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Whether the cursor row is marked (on by default): off for a table only shown, never moved in (a
    /// preview of a folder).
    pub fn cursor(mut self, cursor: bool) -> Self {
        self.cursor = cursor;
        self
    }

    /// A header line with the column titles and the sort marker, above the rows.
    pub fn header(mut self, header: bool) -> Self {
        self.header = header;
        self
    }

    /// What the table shows, muted, when it has no rows.
    pub fn empty(mut self, text: &'l str) -> Self {
        self.empty = Some(text);
        self
    }

    /// Rows that stay at the top while the table scrolls under them: `pinned(first)` names them for the
    /// first scrolling row `first`. At least one row always scrolls. A tree gets its ancestors pinned by
    /// `tree` without this.
    pub fn pinned(mut self, pinned: impl Fn(usize) -> Vec<usize> + 'l) -> Self {
        self.pinned = Some(Box::new(pinned));
        self
    }

    /// The rows are a tree and `depth(row)` is how deep each is: the table draws the indent guides in the
    /// first flexible column, highlights the scope the cursor is in and pins the rows that enclose the
    /// first scrolling one.
    pub fn tree(mut self, depth: impl Fn(usize) -> usize + 'l) -> Self {
        self.depth = Some(Box::new(depth));
        self
    }
}

impl<F: FnMut(usize) -> Row> Component for TableView<'_, F> {
    fn render(mut self, cx: &Context, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let columns = self.columns;
        let header = self.header && area.height > 1;
        let body =
            if header { Rect { y: area.y + 1, height: area.height - 1, ..area } } else { area };
        let room = usize::from(body.width);
        let table = &mut *self.table;
        let page = usize::from(body.height);
        table.page = page;
        table.shown.clear();
        table.cols = columns.len();
        if let Some(col) = table.col {
            table.col = Some(col.min(columns.len().saturating_sub(1)));
        }
        if table.len == 0 {
            if header {
                let wants = wants(table, columns, &[], header);
                let placed = resolve(columns, &wants, room, table.overflow, table.first_col);
                row::header(cx, buf, area, columns, &placed, table.sort);
            }
            if let Some(text) = self.empty {
                let style = Style::new().fg(cx.theme.colors.muted.into());
                buf.set_stringn(body.x + 1, body.y, text, room, style);
            }
            return;
        }
        table.clamp();
        let depth = self.depth.as_deref();
        let pinned = |first: usize| {
            let mut rows = match (&self.pinned, depth) {
                (Some(f), _) => f(first),
                (None, Some(d)) => tree::ancestors_by(d, first),
                (None, None) => Vec::new(),
            };
            rows.drain(..rows.len().saturating_sub(page - 1));
            rows
        };
        let mut first =
            follow_cursor(table.cursor, table.first_visible_row, page, table.len, table.scroll_off);
        // Pinned rows take lines from the window: scroll on until the cursor and its margin fit below them.
        let want = (table.cursor + scroll_margin(page, table.scroll_off)).min(table.len - 1);
        while first < table.cursor && want >= first + page - pinned(first).len() {
            first += 1;
        }
        table.first_visible_row = first;
        let top = pinned(first);
        let scrolling = first..(first + page - top.len()).min(table.len);
        table.shown = top.into_iter().chain(scrolling).collect();

        let scope = depth.and_then(|d| tree::active_scope_by(table.len, d, table.cursor));
        let guide_col = columns
            .iter()
            .position(|c| c.kind == Kind::Cells && matches!(c.width, Width::Flex(_)))
            .or_else(|| columns.iter().position(|c| c.kind == Kind::Cells));
        let rows: Vec<(usize, Row, Vec<Option<Cell>>)> = table
            .shown
            .iter()
            .map(|&index| {
                let row = (self.row)(index);
                let mut cells = row.cells_for(cx, columns);
                if let (Some(d), Some(k)) = (depth, guide_col) {
                    let guides =
                        tree::guides_marked(cx, d(index), scope.and_then(|s| s.col_at(index)));
                    cells[k].get_or_insert_with(Cell::default).prepend(guides);
                }
                (index, row, cells)
            })
            .collect();
        let cells: Vec<&[Option<Cell>]> = rows.iter().map(|(_, _, c)| c.as_slice()).collect();
        let wants = wants(table, columns, &cells, header);
        if table.overflow == Overflow::Scroll
            && let Some(col) = table.col
        {
            // Scroll sideways as little as keeps the cursor's column whole on screen.
            table.first_col = table.first_col.min(col);
            let span = |from: usize| wants[from..=col].iter().sum::<usize>() + (col - from) * GAP;
            while table.first_col < col && span(table.first_col) > room {
                table.first_col += 1;
            }
        }
        let placed = resolve(columns, &wants, room, table.overflow, table.first_col);
        if header {
            row::header(cx, buf, area, columns, &placed, table.sort);
        }
        let geometry = Rows::new(self.pane.unwrap_or(area), body);
        for (line, (index, row, cells)) in rows.into_iter().enumerate() {
            let y = body.y + line as u16;
            let state = State {
                is_selected: self.cursor && index == table.cursor,
                is_focused: self.focused,
                cell: table.col,
            };
            let base = row::bar_style(cx, row.tint, state);
            let (id, content) = row.identity();
            let at = row::Placement { columns, placed: &placed };
            row::paint(cx, buf, (&geometry, y), &at, cells, base, state);
            cx.motion.note_row(geometry.id(), y, id, content, geometry.bar(y));
        }
    }
}

/// The width each column wants this frame: a fixed width, a fit width grown to the widest cell (and
/// title) so far, or a flexible column's minimum plus the widest part of its cells that does not shrink.
fn wants(
    table: &mut Table,
    columns: &[Column],
    rows: &[&[Option<Cell>]],
    header: bool,
) -> Vec<usize> {
    columns
        .iter()
        .enumerate()
        .map(|(i, column)| {
            let cells = rows.iter().filter_map(|r| r.get(i).and_then(Option::as_ref));
            let min = usize::from(column.min);
            match column.width {
                Width::Fixed(n) => usize::from(n),
                Width::Fit { max } => {
                    let seen = cells.map(Cell::width).max().unwrap_or(0);
                    let title = if header { title_width(column) } else { 0 };
                    let kept =
                        table.fit.iter().find(|(id, _)| *id == column.id).map_or(0, |(_, w)| *w);
                    let w = (seen.max(title).min(usize::from(max)) as u16).max(kept);
                    match table.fit.iter_mut().find(|(id, _)| *id == column.id) {
                        Some((_, kept)) => *kept = w,
                        None => table.fit.push((column.id, w)),
                    }
                    usize::from(w).max(min)
                }
                Width::Flex(_) => min + cells.map(Cell::fixed_width).max().unwrap_or(0),
            }
        })
        .collect()
}
