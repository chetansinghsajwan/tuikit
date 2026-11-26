//! How a table scrolls: the one rule that keeps the cursor in view with a margin around it.

/// The first visible row that keeps `cursor` inside a window of `page` rows of a table of `len`
/// rows, `scroll_margin` rows from both edges (except at the ends), scrolling as little
/// as possible from `first_visible_row`, and never so far that the window runs past the last row (a
/// table that shrank under its old scroll). The one rule every table scrolls by.
pub(crate) fn follow_cursor(
    cursor: usize,
    first_visible_row: usize,
    page: usize,
    len: usize,
    scroll_off: usize,
) -> usize {
    let margin = scroll_margin(page, scroll_off);
    let last = len.saturating_sub(1);
    let first_visible_row = first_visible_row.min(len.saturating_sub(page));
    if cursor < first_visible_row + margin {
        cursor.saturating_sub(margin)
    } else if (cursor + margin).min(last) >= first_visible_row + page {
        (cursor + margin).min(last) + 1 - page
    } else {
        first_visible_row
    }
}

/// The rows kept between the cursor and the edge of a window of `page` rows: `scroll_off`, less in a
/// window too small to hold it on both sides of the cursor.
pub(crate) fn scroll_margin(page: usize, scroll_off: usize) -> usize {
    scroll_off.min(page.saturating_sub(1) / 2)
}
