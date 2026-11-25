//! Tests for list rows: bars, text fitting and clipping.

use super::*;
use crate::Theme;

#[test]
fn the_bar_reaches_the_borders_and_the_content_stays_inside() {
    let pane = Rect::new(0, 0, 20, 6);
    let inner = Rect::new(2, 1, 16, 4); // border 1 + padding 1 on each side
    let rows = Rows::new(pane, inner);
    assert_eq!(rows.bar(2), Rect::new(1, 2, 18, 1));
    assert_eq!(rows.content(2), Rect::new(2, 2, 16, 1));
    // No padding: the bar is the content.
    let rows = Rows::new(pane, Rect::new(1, 1, 18, 4));
    assert_eq!(rows.bar(1), rows.content(1));
}

#[test]
fn hovered_beats_tint_and_plain_rows_have_no_background() {
    let t = Theme::default();
    assert!(style(&t, true, Some(Color::Red)).bg.is_some());
    assert_ne!(style(&t, true, None).bg, style(&t, false, Some(Color::Red)).bg);
    assert_eq!(style(&t, false, None).bg, None);
}

#[test]
fn the_cursor_stops_at_the_first_and_the_last_row() {
    assert_eq!(move_cursor(2, -5, 10), 0);
    assert_eq!(move_cursor(2, 50, 10), 9);
    assert_eq!(move_cursor(2, 3, 10), 5);
    assert_eq!(move_cursor(0, 3, 0), 0);
}

#[test]
fn the_window_scrolls_only_as_far_as_the_cursor_needs() {
    assert_eq!(follow_cursor(5, 0, 10, 50, SCROLL_OFF), 0);
    assert_eq!(follow_cursor(12, 0, 10, 50, SCROLL_OFF), 7);
    assert_eq!(follow_cursor(1, 4, 10, 50, SCROLL_OFF), 0);
    assert_eq!(follow_cursor(13, 4, 10, 50, SCROLL_OFF), 8);
}

#[test]
fn the_window_keeps_the_margin_but_not_past_the_ends() {
    assert_eq!(follow_cursor(5, 0, 10, 50, SCROLL_OFF), 0);
    assert_eq!(follow_cursor(6, 0, 10, 50, SCROLL_OFF), 1);
    assert_eq!(follow_cursor(7, 5, 10, 50, SCROLL_OFF), 3);
    assert_eq!(follow_cursor(49, 0, 10, 50, SCROLL_OFF), 40);
    assert_eq!(follow_cursor(0, 3, 10, 50, SCROLL_OFF), 0);
}

#[test]
fn the_scroll_margin_shrinks_in_a_small_window() {
    assert_eq!(scroll_margin(30, SCROLL_OFF), 4);
    assert_eq!(scroll_margin(5, SCROLL_OFF), 2);
    assert_eq!(scroll_margin(1, SCROLL_OFF), 0);
}

#[test]
fn a_smaller_scroll_off_keeps_fewer_rows_ahead() {
    assert_eq!(scroll_margin(30, 1), 1);
    assert_eq!(follow_cursor(12, 0, 10, 50, 1), 4);
}

/// Draws `list` with `len` rows named by index into a 10 by `height` area, with its own motion.
fn draw(list: &mut List, len: usize, height: u16, focused: bool) -> ratatui::buffer::Buffer {
    draw_pinned(list, len, height, focused, |_| Vec::new())
}

/// `draw` with `pinned` naming the rows pinned above the first scrolling row.
fn draw_pinned(
    list: &mut List,
    len: usize,
    height: u16,
    focused: bool,
    pinned: impl Fn(usize) -> Vec<usize>,
) -> ratatui::buffer::Buffer {
    use crate::component::{Component, Context};
    let theme = Theme::default();
    let motion = crate::anim::Motion::new();
    let area = Rect::new(0, 0, 10, height);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    let row =
        |s: Slot| ListRow { left: vec![Span::raw(format!("r{}", s.index))], ..Default::default() };
    list.view(len, row).focused(focused).empty("none").pinned(pinned).render(
        &Context::with_motion(&theme, &motion),
        area,
        &mut buf,
    );
    buf
}

/// The text on line `y` of `buf`, trimmed.
fn line(buf: &ratatui::buffer::Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect::<String>().trim().to_string()
}

#[test]
fn a_list_scrolls_to_keep_the_cursor_in_view() {
    let mut list = List::new();
    list.set_len(20);
    list.set_cursor(15);
    let buf = draw(&mut list, 20, 10, true);
    assert_eq!(list.first_visible_row(), 10, "the cursor keeps its margin from the bottom");
    assert_eq!(line(&buf, 0), "r10");
    assert_eq!(line(&buf, 5), "r15");
    assert_eq!(list.page_row_count(), 10);
}

#[test]
fn a_list_marks_the_cursor_row_and_the_focus() {
    let theme = Theme::default();
    let mut list = List::new();
    list.set_len(3);
    list.set_cursor(1);
    let focused = draw(&mut list, 3, 3, true);
    assert_eq!(focused[(0, 1)].bg, style(&theme, true, None).bg.unwrap());
    assert_ne!(focused[(0, 0)].bg, focused[(0, 1)].bg);
    let unfocused = draw(&mut list, 3, 3, false);
    assert_eq!(unfocused[(0, 1)].bg, active(&theme).bg.unwrap());
}

#[test]
fn a_list_that_shrinks_keeps_the_cursor_inside_and_shows_its_empty_text() {
    let mut list = List::new();
    list.set_len(10);
    list.set_cursor(9);
    draw(&mut list, 10, 3, true);
    draw(&mut list, 3, 3, true);
    assert_eq!((list.cursor(), list.first_visible_row()), (2, 0));
    let buf = draw(&mut list, 0, 3, true);
    assert_eq!(line(&buf, 0), "none");
}

#[test]
fn the_cursor_skips_rows_it_can_not_land_on() {
    let mut list = List::new();
    // Rows 0 and 3 are group headers.
    list.set_rows(6, |i| i != 0 && i != 3);
    assert_eq!(list.cursor(), 1, "the cursor starts on the first row it can take");
    list.move_by(2);
    assert_eq!(list.cursor(), 4);
    list.move_to_top();
    assert_eq!(list.cursor(), 1);
    list.move_to_bottom();
    assert_eq!(list.cursor(), 5);
    list.set_cursor(3);
    assert_eq!(list.cursor(), 2, "a header falls back to the row above it");
}

#[test]
fn pinned_rows_stay_on_top_and_the_cursor_stays_below_them() {
    let mut list = List::new().with_scroll_off(0);
    list.set_len(30);
    list.set_cursor(20);
    // Rows 0 and 1 enclose everything after them.
    let buf = draw_pinned(&mut list, 30, 5, true, |first| (0..first.min(2)).collect());
    assert_eq!(line(&buf, 0), "r0");
    assert_eq!(line(&buf, 1), "r1");
    assert_eq!(line(&buf, 4), "r20", "the cursor is the last line, below the pinned rows");
}
