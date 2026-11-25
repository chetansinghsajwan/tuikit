//! Tests for list rows: bars, text fitting and clipping.

use super::*;

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
    assert_eq!(follow_cursor(5, 0, 10, 50), 0);
    assert_eq!(follow_cursor(12, 0, 10, 50), 7);
    assert_eq!(follow_cursor(1, 4, 10, 50), 0);
    assert_eq!(follow_cursor(13, 4, 10, 50), 8);
}

#[test]
fn the_window_keeps_the_margin_but_not_past_the_ends() {
    assert_eq!(follow_cursor(5, 0, 10, 50), 0);
    assert_eq!(follow_cursor(6, 0, 10, 50), 1);
    assert_eq!(follow_cursor(7, 5, 10, 50), 3);
    assert_eq!(follow_cursor(49, 0, 10, 50), 40);
    assert_eq!(follow_cursor(0, 3, 10, 50), 0);
}

#[test]
fn the_scroll_margin_shrinks_in_a_small_window() {
    assert_eq!(scroll_margin(30), 4);
    assert_eq!(scroll_margin(5), 2);
    assert_eq!(scroll_margin(1), 0);
}

/// Draws a list of `len` rows named by index into a 10 by `height` area, with its own motion.
fn draw_list(
    len: usize,
    height: u16,
    state: &mut ListState,
    focused: bool,
) -> ratatui::buffer::Buffer {
    use crate::component::{Component, Context};
    let theme = Theme::default();
    let motion = crate::anim::Motion::new();
    let area = Rect::new(0, 0, 10, height);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    let row = |i: usize| ListRow { left: vec![Span::raw(format!("r{i}"))], ..Default::default() };
    List::new(len, state, row).focused(focused).empty("none").render(
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
    let mut state = ListState { cursor: 15, first_visible_row: 0 };
    let buf = draw_list(20, 10, &mut state, true);
    assert_eq!(state.first_visible_row, 10, "the cursor keeps its margin from the bottom");
    assert_eq!(line(&buf, 0), "r10");
    assert_eq!(line(&buf, 5), "r15");
}

#[test]
fn a_list_marks_the_cursor_row_and_the_focus() {
    let theme = Theme::default();
    let mut state = ListState { cursor: 1, first_visible_row: 0 };
    let focused = draw_list(3, 3, &mut state, true);
    assert_eq!(focused[(0, 1)].bg, style(&theme, true, None).bg.unwrap());
    assert_ne!(focused[(0, 0)].bg, focused[(0, 1)].bg);
    let unfocused = draw_list(3, 3, &mut state, false);
    assert_eq!(unfocused[(0, 1)].bg, active(&theme).bg.unwrap());
}

#[test]
fn a_list_clamps_a_stale_cursor_and_shows_its_empty_text() {
    let mut state = ListState { cursor: 9, first_visible_row: 7 };
    draw_list(3, 3, &mut state, true);
    assert_eq!(state, ListState { cursor: 2, first_visible_row: 0 });
    let buf = draw_list(0, 3, &mut ListState::default(), true);
    assert_eq!(line(&buf, 0), "none");
}
