//! `List`: the component that draws a scrolling list of rows, so no tool repeats the window, the
//! scrolling and the cursor marks.

use super::{ListRow, Rows, follow_cursor};
use crate::component::{Component, Context};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

/// Where the cursor is in a list and how far the list is scrolled. The tool owns it and moves the
/// cursor; `List` scrolls it to keep the cursor in view.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ListState {
    pub cursor: usize,
    pub first_visible_row: usize,
}

/// A list of `len` rows drawn into the area it is given: it scrolls `state` to keep the cursor in
/// view, asks `row` for each visible row by index and marks the cursor row and the focus itself.
pub struct List<'s, F> {
    len: usize,
    state: &'s mut ListState,
    row: F,
    pane: Option<Rect>,
    focused: bool,
    empty: Option<&'s str>,
}

impl<'s, F: FnMut(usize) -> ListRow> List<'s, F> {
    /// A list of `len` rows at `state`, unfocused, whose row `i` is `row(i)`.
    pub fn new(len: usize, state: &'s mut ListState, row: F) -> Self {
        List { len, state, row, pane: None, focused: false, empty: None }
    }

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
    pub fn empty(mut self, text: &'s str) -> Self {
        self.empty = Some(text);
        self
    }
}

impl<F: FnMut(usize) -> ListRow> Component for List<'_, F> {
    fn render(mut self, cx: &Context, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        if self.len == 0 {
            if let Some(text) = self.empty {
                let style = Style::new().fg(cx.theme.colors.muted.into());
                buf.set_stringn(area.x + 1, area.y, text, usize::from(area.width), style);
            }
            return;
        }
        let page = usize::from(area.height);
        let st = &mut *self.state;
        st.cursor = st.cursor.min(self.len - 1);
        st.first_visible_row = follow_cursor(st.cursor, st.first_visible_row, page, self.len);
        let rows = Rows::new(self.pane.unwrap_or(area), area);
        let (cursor, first) = (st.cursor, st.first_visible_row);
        for (line, i) in (first..(first + page).min(self.len)).enumerate() {
            let mut row = (self.row)(i);
            row.is_selected = i == cursor;
            row.is_focused = self.focused;
            row.render(cx, buf, &rows, area.y + line as u16);
        }
    }
}
