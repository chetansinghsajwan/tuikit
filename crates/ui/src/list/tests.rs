//! Tests for the shared list rules: bar styles and cursor moves.

use super::*;
use crate::Theme;

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
