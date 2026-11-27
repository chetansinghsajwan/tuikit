//! Tests for the graph and the meter.

use super::*;
use crate::Theme;

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
    assert_eq!(buf[(0, 0)].symbol(), "━");
    assert_eq!(buf[(1, 0)].symbol(), "━");
    assert_eq!(buf[(2, 0)].symbol(), "─");
}

#[test]
fn meter_ends_in_a_half_cell_and_never_hides_a_small_load() {
    let area = Rect::new(0, 0, 4, 1);
    let mut buf = Buffer::empty(area);
    meter(&mut buf, area, 0.375, |_| Color::Green, Color::DarkGray);
    assert_eq!(buf[(0, 0)].symbol(), "━");
    assert_eq!(buf[(1, 0)].symbol(), "╸");
    assert_eq!(buf[(2, 0)].symbol(), "─");
    meter(&mut buf, area, 0.001, |_| Color::Green, Color::DarkGray);
    assert_eq!(buf[(0, 0)].symbol(), "╸");
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

#[test]
fn a_meter_row_keeps_a_blank_cell_between_its_parts() {
    let theme = Theme::default();
    let cx = crate::Context::new(&theme);
    let area = Rect::new(0, 0, 20, 1);
    let mut buf = Buffer::empty(area);
    crate::Component::render(
        MeterRow {
            label: "7",
            label_width: 3,
            ratio: Some(0.5),
            value: "50%",
            value_width: 4,
            reversed: false,
        },
        &cx,
        area,
        &mut buf,
    );
    let row: String = (0..20).map(|x| buf[(x, 0)].symbol().to_string()).collect();
    assert!(row.starts_with("7   "), "label, its width, then one blank: {row:?}");
    assert_eq!(&row[row.len() - 5..], "  50%", "one blank before the value");
}

#[test]
fn an_unavailable_meter_row_is_an_empty_track_and_a_muted_value() {
    let theme = Theme::default();
    let cx = crate::Context::new(&theme);
    let area = Rect::new(0, 0, 20, 1);
    let mut buf = Buffer::empty(area);
    crate::Component::render(
        MeterRow {
            label: "battery",
            label_width: 7,
            ratio: None,
            value: "n/a",
            value_width: 4,
            reversed: false,
        },
        &cx,
        area,
        &mut buf,
    );
    let row: String = (0..20).map(|x| buf[(x, 0)].symbol().to_string()).collect();
    assert!(row.contains('─') && !row.contains('━'), "{row:?}");
    assert!(row.ends_with(" n/a"), "{row:?}");
    assert_eq!(buf[(19, 0)].fg, Color::from(theme.colors.muted));
}

#[test]
fn history_pair_blends_where_both_overlap_and_keeps_the_taller_color_above() {
    let area = Rect::new(0, 0, 1, 2);
    let mut buf = Buffer::empty(area);
    let (a, b) = (Color::Rgb(200, 0, 0), Color::Rgb(0, 0, 200));
    history_pair(&mut buf, area, (&[1.0], &[0.5]), (a, b));
    assert_eq!(buf[(0, 1)].symbol(), "█");
    assert_eq!(buf[(0, 1)].fg, Color::Rgb(100, 0, 100), "both fill the lower cell");
    assert_eq!(buf[(0, 0)].symbol(), "█");
    assert_eq!(buf[(0, 0)].fg, a, "only the taller one reaches the upper cell");
}

#[test]
fn history_pair_draws_a_series_alone_in_its_own_color() {
    let area = Rect::new(0, 0, 2, 1);
    let mut buf = Buffer::empty(area);
    history_pair(&mut buf, area, (&[0.5, 0.0], &[0.0, 1.0]), (Color::Red, Color::Blue));
    assert_eq!(buf[(0, 0)].symbol(), "▄");
    assert_eq!(buf[(0, 0)].fg, Color::Red);
    assert_eq!(buf[(1, 0)].symbol(), "█");
    assert_eq!(buf[(1, 0)].fg, Color::Blue, "only the second series has a reading");
}

#[test]
fn axis_labels_the_top_and_bottom_and_returns_the_graph_area() {
    let theme = Theme::default();
    let cx = crate::Context::new(&theme);
    let area = Rect::new(0, 0, 12, 3);
    let mut buf = Buffer::empty(area);
    let rest = axis(&cx, &mut buf, area, "1.2M/s", "0");
    assert_eq!(rest, Rect::new(7, 0, 5, 3));
    let top: String = (0..6).map(|x| buf[(x, 0)].symbol().to_string()).collect();
    assert_eq!(top, "1.2M/s");
    assert_eq!(buf[(5, 2)].symbol(), "0", "right aligned under the top label");
    let narrow = Rect::new(0, 0, 8, 3);
    assert_eq!(axis(&cx, &mut buf, narrow, "1.2M/s", "0"), narrow, "no room: no scale");
}
