//! Tests for the graph and the meter.

use super::*;

#[test]
fn a_small_load_still_shows_one_eighth() {
    assert_eq!(filled_eighths(0.001, 4), 1);
    assert_eq!(filled_eighths(0.0, 4), 0);
    assert_eq!(filled_eighths(1.0, 4), 32);
    assert_eq!(filled_eighths(f32::NAN, 4), 0);
}

#[test]
fn history_puts_the_newest_value_at_the_right_edge() {
    let area = Rect::new(0, 0, 4, 1);
    let mut buf = Buffer::empty(area);
    history(&mut buf, area, &[1.0, 0.5], Color::Red);
    assert_eq!(buf[(2, 0)].symbol(), "█");
    assert_eq!(buf[(3, 0)].symbol(), "▄");
    assert_eq!(buf[(0, 0)].symbol(), " ");
}

#[test]
fn history_stacks_rows_for_a_tall_area() {
    let area = Rect::new(0, 0, 1, 2);
    let mut buf = Buffer::empty(area);
    history(&mut buf, area, &[0.75], Color::Red);
    assert_eq!(buf[(0, 1)].symbol(), "█");
    assert_eq!(buf[(0, 0)].symbol(), "▄");
}

#[test]
fn history_drops_the_oldest_values_that_do_not_fit() {
    let area = Rect::new(0, 0, 1, 1);
    let mut buf = Buffer::empty(area);
    history(&mut buf, area, &[1.0, 0.0, 0.5], Color::Red);
    assert_eq!(buf[(0, 0)].symbol(), "▄");
}

#[test]
fn meter_fills_from_the_left_and_leaves_a_track() {
    let area = Rect::new(0, 0, 4, 1);
    let mut buf = Buffer::empty(area);
    meter(&mut buf, area, 0.5, Color::Green, Color::DarkGray);
    assert_eq!(buf[(0, 0)].symbol(), "█");
    assert_eq!(buf[(1, 0)].symbol(), "█");
    assert_eq!(buf[(2, 0)].symbol(), "▁");
}
