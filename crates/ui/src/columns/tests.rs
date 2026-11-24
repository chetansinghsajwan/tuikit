//! Tests for the columns.

use super::*;

fn columns() -> Columns {
    Columns::new(
        vec![Column::right("PID", 5)],
        "NAME",
        vec![Column::right("CPU", 4), Column::left("STATE", 6)],
        8,
    )
}

fn text(spans: &[Span]) -> String {
    spans.iter().map(|s| s.content.as_ref()).collect()
}

fn header(columns: &Columns, width: u16) -> String {
    let area = Rect::new(0, 0, width, 1);
    let mut buf = Buffer::empty(area);
    columns.draw_header(&Theme::default(), &mut buf, area);
    (0..width).map(|x| buf[(x, 0)].symbol().to_string()).collect()
}

#[test]
fn cells_are_padded_to_their_width_by_alignment() {
    let c = columns();
    assert_eq!(text(&c.lead_cells(vec![Span::raw("42")])), "   42 ");
    assert_eq!(text(&c.trail_cells(vec![Span::raw("7"), Span::raw("Run")])), "   7 Run   ");
}

#[test]
fn a_cell_too_wide_is_cut_with_an_ellipsis_and_a_missing_one_is_blank() {
    let c = columns();
    assert_eq!(
        text(&c.trail_cells(vec![Span::raw("12345"), Span::raw("Sleeping")])),
        "123… Sleep…"
    );
    assert_eq!(text(&c.trail_cells(vec![])).len(), 11);
}

#[test]
fn the_trail_is_as_wide_as_the_header_says() {
    let c = columns();
    let row = c.trail_cells(vec![Span::raw("1"), Span::raw("x")]);
    assert_eq!(text(&row).width(), c.trail_width());
}

#[test]
fn the_header_lines_up_with_the_rows() {
    let c = columns();
    // the lead "  PID " then the name; the trail ends at the right edge
    assert_eq!(header(&c, 30), "  PID NAME          CPU STATE ");
}

#[test]
fn the_sort_column_gets_a_marker() {
    let c = columns().sorted_by("CPU", true);
    assert!(header(&c, 30).contains("CPU▼"));
    let c = columns().sorted_by("NAME", false);
    assert!(header(&c, 30).contains("NAME▲"));
}

#[test]
fn the_trail_is_dropped_when_it_would_leave_the_left_side_too_little() {
    let c = columns();
    // trail 11 + 1 blank + 8 for the left = 20
    assert!(header(&c, 20).contains("STATE"));
    assert!(!header(&c, 19).contains("STATE"));
}
