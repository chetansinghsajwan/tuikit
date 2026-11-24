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
    history(&mut buf, area, &[1.0, 0.5], |_| Color::Red);
    assert_eq!(buf[(2, 0)].symbol(), "█");
    assert_eq!(buf[(3, 0)].symbol(), "▄");
    assert_eq!(buf[(0, 0)].symbol(), " ");
}

#[test]
fn history_stacks_rows_for_a_tall_area() {
    let area = Rect::new(0, 0, 1, 2);
    let mut buf = Buffer::empty(area);
    history(&mut buf, area, &[0.75], |_| Color::Red);
    assert_eq!(buf[(0, 1)].symbol(), "█");
    assert_eq!(buf[(0, 0)].symbol(), "▄");
}

#[test]
fn history_drops_the_oldest_values_that_do_not_fit() {
    let area = Rect::new(0, 0, 1, 1);
    let mut buf = Buffer::empty(area);
    history(&mut buf, area, &[1.0, 0.0, 0.5], |_| Color::Red);
    assert_eq!(buf[(0, 0)].symbol(), "▄");
}

#[test]
fn meter_fills_from_the_left_and_leaves_a_track() {
    let area = Rect::new(0, 0, 4, 1);
    let mut buf = Buffer::empty(area);
    meter(&mut buf, area, 0.5, |_| Color::Green, Color::DarkGray);
    assert_eq!(buf[(0, 0)].symbol(), "█");
    assert_eq!(buf[(1, 0)].symbol(), "█");
    assert_eq!(buf[(2, 0)].symbol(), "▁");
}

#[test]
fn eased_starts_at_the_old_value_and_ends_at_the_target() {
    let t0 = Instant::now();
    let mut e = Eased::new(0.0);
    e.set(1.0, t0);
    assert!(e.is_moving(t0));
    assert_eq!(e.value(t0), 0.0);
    let mid = e.value(t0 + GLIDE / 2);
    assert!(mid > 0.0 && mid < 1.0);
    assert!(!e.is_moving(t0 + GLIDE));
    assert_eq!(e.value(t0 + GLIDE), 1.0);
}

#[test]
fn eased_retargets_from_where_it_is() {
    let t0 = Instant::now();
    let mut e = Eased::new(0.0);
    e.set(1.0, t0);
    let half = t0 + GLIDE / 2;
    let at = e.value(half);
    e.set(0.0, half);
    assert_eq!(e.value(half), at);
    assert_eq!(e.value(half + GLIDE), 0.0);
}

#[test]
fn eased_with_the_same_target_does_not_restart() {
    let t0 = Instant::now();
    let mut e = Eased::new(0.5);
    e.set(0.5, t0);
    assert!(!e.is_moving(t0));
}

#[test]
fn heat_runs_from_success_through_warning_to_error() {
    let theme = Theme::default();
    let c = &theme.colors;
    assert_eq!(heat(&theme, 0.0), c.success.into());
    assert_eq!(heat(&theme, 0.5), c.warning.into());
    assert_eq!(heat(&theme, 1.0), c.error.into());
    assert_eq!(heat(&theme, f32::NAN), c.success.into());
    let mid = heat(&theme, 0.25);
    assert_ne!(mid, c.success.into());
    assert_ne!(mid, c.warning.into());
}

#[test]
fn meter_colors_each_cell_by_its_place() {
    let area = Rect::new(0, 0, 2, 1);
    let mut buf = Buffer::empty(area);
    meter(&mut buf, area, 1.0, |p| if p < 0.5 { Color::Green } else { Color::Red }, Color::Gray);
    assert_eq!(buf[(0, 0)].fg, Color::Green);
    assert_eq!(buf[(1, 0)].fg, Color::Red);
}

#[test]
fn history_colors_each_column_by_its_value() {
    let area = Rect::new(0, 0, 2, 1);
    let mut buf = Buffer::empty(area);
    history(&mut buf, area, &[0.2, 0.9], |v| if v < 0.5 { Color::Green } else { Color::Red });
    assert_eq!(buf[(0, 0)].fg, Color::Green);
    assert_eq!(buf[(1, 0)].fg, Color::Red);
}
