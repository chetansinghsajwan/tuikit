//! `List`: the one scrolling list of rows. A tool keeps a `List` between frames (the cursor, the
//! scroll, how far the cursor stays from the edges) and draws it each frame with `List::view`, so no
//! tool repeats the window, the scrolling, the pinned rows or the cursor marks.

use super::{ListRow, Rows, SCROLL_OFF, follow_cursor, move_cursor};
use crate::app::Navigable;
use crate::component::{Component, Context};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

/// A scrolling list the tool keeps: where the cursor is, how far the list is scrolled, and which rows
/// the cursor can land on (all of them, unless the tool says otherwise with `set_rows`). It is
/// `Navigable`, so the shell's movement actions move it. Rows are drawn with `view`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    cursor: usize,
    first_visible_row: usize,
    /// How many rows stay visible between the cursor and the edge while scrolling (vim's `scrolloff`).
    scroll_off: usize,
    /// The rows the last frame showed, for page moves.
    page: usize,
    len: usize,
    /// The rows the cursor can land on, ascending; `None` when every row can take it.
    selectable: Option<Vec<usize>>,
    /// The rows the last frame drew, top to bottom (pinned rows first).
    shown: Vec<usize>,
}

impl Default for List {
    fn default() -> Self {
        List {
            cursor: 0,
            first_visible_row: 0,
            scroll_off: SCROLL_OFF,
            page: 1,
            len: 0,
            selectable: None,
            shown: Vec::new(),
        }
    }
}

impl List {
    /// An empty list with the cursor on the first row and the default `scroll_off`.
    pub fn new() -> List {
        List::default()
    }

    /// Keep `rows` rows between the cursor and the edge while scrolling, instead of `SCROLL_OFF`.
    pub fn with_scroll_off(mut self, rows: usize) -> List {
        self.scroll_off = rows;
        self
    }

    /// The row the cursor is on.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The first row that scrolls (below any pinned rows).
    pub fn first_visible_row(&self) -> usize {
        self.first_visible_row
    }

    /// How many rows the list has, as last told by `set_len`, `set_rows` or `view`.
    pub fn len(&self) -> usize {
        self.len
    }

    /// The rows the last frame drew, top to bottom with the pinned rows first: what is on screen, for
    /// work that only matters there (counting the sizes of the folders shown).
    pub fn shown(&self) -> &[usize] {
        &self.shown
    }

    /// Whether the list has no rows.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The list now has `len` rows, all of which the cursor can land on; the cursor stays inside it.
    pub fn set_len(&mut self, len: usize) {
        self.len = len;
        self.selectable = None;
        self.clamp();
    }

    /// The list now has `len` rows, of which the cursor lands only on those `can_select` accepts
    /// (not on group headers); the cursor moves to the nearest such row.
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

    /// Back to the top: the cursor on the first row, nothing scrolled.
    pub fn reset(&mut self) {
        self.first_visible_row = 0;
        self.cursor = 0;
        self.clamp();
    }

    /// The rows the cursor can land on, in order.
    fn stops(&self) -> Vec<usize> {
        self.selectable.clone().unwrap_or_else(|| (0..self.len).collect())
    }

    /// Keeps the cursor inside the list and on a row it can land on, the nearest one at or above it
    /// first.
    fn clamp(&mut self) {
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

    /// Draws the list's `len` rows, asking `row` for each visible one by its index and width. The cursor
    /// row and the focus are marked by the list.
    pub fn view<F: FnMut(Slot) -> ListRow>(&mut self, len: usize, row: F) -> ListView<'_, F> {
        if len != self.len {
            self.set_len(len);
        }
        ListView { list: self, row, pane: None, focused: false, empty: None, pinned: None }
    }
}

impl List {
    /// Moves the cursor by `delta` of the rows it can land on, stopping at the first and the last.
    pub fn move_by(&mut self, delta: isize) {
        let stops = self.stops();
        let at = stops.iter().position(|&r| r == self.cursor).unwrap_or(0);
        if let Some(&r) = stops.get(move_cursor(at, delta, stops.len())) {
            self.cursor = r;
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

impl Navigable for List {
    fn move_by(&mut self, delta: isize) {
        List::move_by(self, delta);
    }

    fn page_row_count(&self) -> usize {
        List::page_row_count(self)
    }

    fn move_to_top(&mut self) {
        List::move_to_top(self);
    }

    fn move_to_bottom(&mut self) {
        List::move_to_bottom(self);
    }
}

/// What a row is asked for: which row and how many cells wide its content is.
#[derive(Debug, Clone, Copy)]
pub struct Slot {
    pub index: usize,
    pub width: usize,
}

/// Rows pinned above the scrolling ones (the folders a tree row is inside), given the first row that
/// scrolls.
type Pinned<'l> = Box<dyn Fn(usize) -> Vec<usize> + 'l>;

/// One frame of a `List`: drawn into the area it is given, it scrolls the list to keep the cursor in
/// view, asks the row closure for each visible row and marks the cursor row and the focus itself.
pub struct ListView<'l, F> {
    list: &'l mut List,
    row: F,
    pane: Option<Rect>,
    focused: bool,
    empty: Option<&'l str>,
    pinned: Option<Pinned<'l>>,
}

impl<'l, F: FnMut(Slot) -> ListRow> ListView<'l, F> {
    /// The whole pane the list sits in, so the row bars reach into its padding (see `Rows`).
    pub fn pane(mut self, pane: Rect) -> Self {
        self.pane = Some(pane);
        self
    }

    /// Whether the list's pane has the focus: the cursor row is then the bright one.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// What the list shows, muted, when it has no rows.
    pub fn empty(mut self, text: &'l str) -> Self {
        self.empty = Some(text);
        self
    }

    /// Rows that stay at the top while the list scrolls under them: `pinned(first)` names them for the
    /// first scrolling row `first` (`ui::tree::ancestors` for a tree). At least one row always scrolls.
    pub fn pinned(mut self, pinned: impl Fn(usize) -> Vec<usize> + 'l) -> Self {
        self.pinned = Some(Box::new(pinned));
        self
    }
}

impl<F: FnMut(Slot) -> ListRow> Component for ListView<'_, F> {
    fn render(mut self, cx: &Context, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let list = &mut *self.list;
        let page = usize::from(area.height);
        list.page = page;
        list.shown.clear();
        if list.len == 0 {
            if let Some(text) = self.empty {
                let style = Style::new().fg(cx.theme.colors.muted.into());
                buf.set_stringn(area.x + 1, area.y, text, usize::from(area.width), style);
            }
            return;
        }
        list.clamp();
        let pinned = |first: usize| match &self.pinned {
            Some(f) => {
                let mut rows = f(first);
                rows.drain(..rows.len().saturating_sub(page - 1));
                rows
            }
            None => Vec::new(),
        };
        let mut first =
            follow_cursor(list.cursor, list.first_visible_row, page, list.len, list.scroll_off);
        // Pinned rows take lines from the window: scroll on until the cursor and its margin fit below them.
        let want = (list.cursor + super::scroll_margin(page, list.scroll_off)).min(list.len - 1);
        while first < list.cursor && want >= first + page - pinned(first).len() {
            first += 1;
        }
        list.first_visible_row = first;
        let top = pinned(first);
        let scrolling = first..(first + page - top.len()).min(list.len);
        let rows = Rows::new(self.pane.unwrap_or(area), area);
        let width = usize::from(area.width);
        list.shown = top.into_iter().chain(scrolling).collect();
        for (line, &index) in list.shown.iter().enumerate() {
            let mut row = (self.row)(Slot { index, width });
            row.is_selected = index == list.cursor;
            row.is_focused = self.focused;
            row.render(cx, buf, &rows, area.y + line as u16);
        }
    }
}
