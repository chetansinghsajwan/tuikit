//! Tests for the table: the layout of the columns, scrolling, the header, the tree, sorting and the
//! cell cursor.

use super::layout::{Placed, resolve};
use super::row::Rows;
use super::scroll::{follow_cursor, scroll_margin};
use super::*;
use crate::Theme;
use crate::component::{Component, Context};
use crate::list::{active, style};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Span;

/// The text on line `y` of `buf`, trimmed.
fn line(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect::<String>().trim_end().to_string()
}

/// A plain list: one flexible column.
fn list() -> Vec<Column> {
    vec![Column::list()]
}

/// Draws `table` with `len` rows named `r<index>` in `columns` into a `width` by `height` area.
fn draw_with(
    table: &mut Table,
    columns: &[Column],
    len: usize,
    (width, height): (u16, u16),
    view: impl FnOnce(
        TableView<'_, Box<dyn FnMut(usize) -> Row>>,
    ) -> TableView<'_, Box<dyn FnMut(usize) -> Row>>,
) -> Buffer {
    let theme = Theme::default();
    let motion = crate::anim::Motion::new();
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let row: Box<dyn FnMut(usize) -> Row> =
        Box::new(|i| Row::new(vec![Cell::from(format!("r{i}"))]));
    view(table.view(len, columns, row).empty("none")).render(
        &Context::with_motion(&theme, &motion),
        area,
        &mut buf,
    );
    buf
}

/// Draws a plain list of `len` rows, `height` lines high and 10 wide.
fn draw(table: &mut Table, len: usize, height: u16, focused: bool) -> Buffer {
    draw_with(table, &list(), len, (10, height), |v| v.focused(focused))
}

#[test]
fn a_table_scrolls_to_keep_the_cursor_in_view() {
    let mut table = Table::new();
    table.set_len(20);
    table.set_cursor(15);
    let buf = draw(&mut table, 20, 10, true);
    assert_eq!(table.first_visible_row(), 10, "the cursor keeps its margin from the bottom");
    assert_eq!(line(&buf, 0), "r10");
    assert_eq!(line(&buf, 5), "r15");
    assert_eq!(table.page_row_count(), 10);
}

#[test]
fn a_table_marks_the_cursor_row_and_the_focus() {
    let theme = Theme::default();
    let mut table = Table::new();
    table.set_len(3);
    table.set_cursor(1);
    let focused = draw(&mut table, 3, 3, true);
    assert_eq!(focused[(0, 1)].bg, style(&theme, true, None).bg.unwrap());
    assert_ne!(focused[(0, 0)].bg, focused[(0, 1)].bg);
    let unfocused = draw(&mut table, 3, 3, false);
    assert_eq!(unfocused[(0, 1)].bg, active(&theme).bg.unwrap());
}

#[test]
fn a_table_that_shrinks_keeps_the_cursor_inside_and_shows_its_empty_text() {
    let mut table = Table::new();
    table.set_len(10);
    table.set_cursor(9);
    draw(&mut table, 10, 3, true);
    draw(&mut table, 3, 3, true);
    assert_eq!((table.cursor(), table.first_visible_row()), (2, 0));
    let buf = draw(&mut table, 0, 3, true);
    assert_eq!(line(&buf, 0), " none");
}

#[test]
fn the_cursor_skips_rows_it_can_not_land_on() {
    let mut table = Table::new();
    // Rows 0 and 3 are group headers.
    table.set_rows(6, |i| i != 0 && i != 3);
    assert_eq!(table.cursor(), 1, "the cursor starts on the first row it can take");
    table.move_by(2);
    assert_eq!(table.cursor(), 4);
    table.move_to_top();
    assert_eq!(table.cursor(), 1);
    table.move_to_bottom();
    assert_eq!(table.cursor(), 5);
    table.set_cursor(3);
    assert_eq!(table.cursor(), 2, "a header falls back to the row above it");
}

#[test]
fn pinned_rows_stay_on_top_and_the_cursor_stays_below_them() {
    let mut table = Table::new().with_scroll_off(0);
    table.set_len(30);
    table.set_cursor(20);
    // Rows 0 and 1 enclose everything after them.
    let buf = draw_with(&mut table, &list(), 30, (10, 5), |v| {
        v.pinned(|first| (0..first.min(2)).collect())
    });
    assert_eq!(line(&buf, 0), "r0");
    assert_eq!(line(&buf, 1), "r1");
    assert_eq!(line(&buf, 4), "r20", "the cursor is the last line, below the pinned rows");
}

#[test]
fn a_tree_gets_its_guides_and_its_ancestors_pinned() {
    let depths = [0, 1, 2, 2, 2, 2, 2, 2];
    let mut table = Table::new().with_scroll_off(0);
    table.set_len(depths.len());
    table.set_cursor(7);
    let buf = draw_with(&mut table, &list(), depths.len(), (12, 4), |v| v.tree(move |i| depths[i]));
    assert_eq!(line(&buf, 0), "r0");
    assert_eq!(line(&buf, 1), "│ r1");
    assert_eq!(line(&buf, 3), "│ │ r7");
}

/// The columns of a process list.
fn procs() -> Vec<Column> {
    vec![
        Column::new("pid", "PID", Width::Fixed(5)).right().sortable(false),
        Column::new("name", "NAME", Width::Flex(1)).min(8).sortable(false),
        Column::new("cpu", "CPU", Width::Fixed(4)).right().priority(1).sortable(true),
        Column::new("state", "STATE", Width::Fixed(6)).priority(2),
    ]
}

/// The offsets and widths `resolve` gives `columns` in `room` cells.
fn placed(columns: &[Column], room: usize) -> Vec<Option<(u16, u16)>> {
    let wants: Vec<usize> = columns
        .iter()
        .map(|c| match c.width {
            Width::Fixed(n) => usize::from(n),
            Width::Fit { max } => usize::from(max),
            Width::Flex(_) => usize::from(c.min),
        })
        .collect();
    resolve(columns, &wants, room, (Overflow::Drop, 0), super::layout::GAP)
        .into_iter()
        .map(|p| p.map(|Placed { x, width }| (x, width)))
        .collect()
}

#[test]
fn the_flexible_column_takes_the_room_left_with_one_blank_between_columns() {
    // 5 + 1 + name + 1 + 4 + 1 + 6 = 30
    assert_eq!(
        placed(&procs(), 30),
        vec![Some((0, 5)), Some((6, 12)), Some((19, 4)), Some((24, 6))]
    );
}

#[test]
fn columns_drop_by_priority_when_they_do_not_fit() {
    // The minimum with every column is 5 + 1 + 8 + 1 + 4 + 1 + 6 = 26.
    assert!(placed(&procs(), 26)[3].is_some());
    let narrow = placed(&procs(), 25);
    assert_eq!(narrow[3], None, "STATE (priority 2) goes first");
    assert!(narrow[2].is_some());
    let narrower = placed(&procs(), 18);
    assert_eq!((narrower[2], narrower[3]), (None, None));
    assert_eq!(narrower[1], Some((6, 12)));
}

#[test]
fn the_header_lines_up_with_the_rows_and_marks_the_sort() {
    let columns = procs();
    let mut table = Table::new().with_sort("cpu", true);
    let theme = Theme::default();
    let motion = crate::anim::Motion::new();
    let area = Rect::new(0, 0, 30, 3);
    let mut buf = Buffer::empty(area);
    let row = |_| Row::new(vec!["42".into(), "init".into(), "1.5".into(), "Run".into()]);
    table.view(2, &columns, row).header(true).render(
        &Context::with_motion(&theme, &motion),
        area,
        &mut buf,
    );
    assert_eq!(line(&buf, 0), "  PID NAME         CPU▼ STATE");
    assert_eq!(line(&buf, 1), "   42 init          1.5 Run");
}

#[test]
fn a_name_shrinks_before_what_sits_beside_it() {
    let cell =
        Cell::new(vec![Span::raw("> "), Span::raw("a-long-name"), Span::raw(" 12")]).shrinking(1);
    let text: String = cell.fit(10).iter().map(|s| s.content.to_string()).collect();
    assert_eq!(text, "> a-lo… 12");
    let cell = Cell::from("a-long-name");
    let text: String = cell.fit(5).iter().map(|s| s.content.to_string()).collect();
    assert_eq!(text, "a-lo…");
}

#[test]
fn a_fit_column_grows_with_what_it_has_seen_and_never_shrinks_back() {
    let columns = vec![Column::list(), Column::new("size", "", Width::Fit { max: 10 }).right()];
    let mut table = Table::new();
    let draw = |table: &mut Table, size: &'static str| {
        let theme = Theme::default();
        let motion = crate::anim::Motion::new();
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        let row = |_| Row::new(vec!["a".into(), size.into()]);
        table.view(1, &columns, row).render(&Context::with_motion(&theme, &motion), area, &mut buf);
        line(&buf, 0)
    };
    assert_eq!(draw(&mut table, "1.2M"), "a               1.2M");
    assert_eq!(draw(&mut table, "3K"), "a                 3K");
    assert_eq!(table.fit, vec![("size", 1, 4)], "the column stays 4 wide");
    table.reset_widths();
    assert!(table.fit.is_empty());
}

#[test]
fn signs_keep_their_cell_whether_a_row_has_one_or_not() {
    let columns = vec![
        Column::list(),
        Column::new("size", "", Width::Fit { max: 10 }).right(),
        Column::signs(),
    ];
    let mut table = Table::new();
    let theme = Theme::default();
    let motion = crate::anim::Motion::new();
    let area = Rect::new(0, 0, 12, 2);
    let mut buf = Buffer::empty(area);
    let row = |i| Row {
        cells: vec!["a".into(), "1K".into()],
        signs: if i == 0 { vec![('M', ratatui::style::Color::Yellow)] } else { Vec::new() },
        ..Row::default()
    };
    table.view(2, &columns, row).render(&Context::with_motion(&theme, &motion), area, &mut buf);
    assert_eq!(line(&buf, 0), "a       1K M");
    assert_eq!(line(&buf, 1), "a       1K");
}

#[test]
fn the_sort_moves_through_the_sortable_columns_in_their_own_direction() {
    let columns = procs();
    let mut table = Table::new().with_sort("cpu", true);
    table.sort_next(&columns);
    assert_eq!(table.sort(), Some(Sort { column: "pid", descending: false }));
    table.sort_next(&columns);
    assert_eq!(table.sort().map(|s| s.column), Some("name"));
    table.sort_next(&columns);
    assert_eq!(table.sort(), Some(Sort { column: "cpu", descending: true }));
    assert!(!table.is_sort_reversed(&columns));
    table.reverse_sort();
    assert!(table.is_sort_reversed(&columns));
}

#[test]
fn marks_hold_the_tools_keys_and_fall_back_to_the_current_row() {
    let mut marks = Marks::new();
    assert_eq!(marks.or_current(Some("a")), vec!["a"]);
    assert!(marks.toggle("b"));
    assert!(marks.toggle("c"));
    assert!(!marks.toggle("b"));
    assert_eq!(marks.or_current(Some("a")), vec!["c"]);
}

#[test]
fn in_cell_mode_the_table_scrolls_sideways_to_the_cursor_column() {
    let columns: Vec<Column> =
        (0..6).map(|i| Column::new("c", format!("C{i}"), Width::Fixed(4))).collect();
    let mut table = Table::new().with_cells();
    let theme = Theme::default();
    let motion = crate::anim::Motion::new();
    let area = Rect::new(0, 0, 14, 2);
    let draw = |table: &mut Table| {
        let mut buf = Buffer::empty(area);
        let row = |_| Row::new((0..6).map(|c| Cell::from(format!("v{c}"))).collect());
        table.view(1, &columns, row).header(true).focused(true).render(
            &Context::with_motion(&theme, &motion),
            area,
            &mut buf,
        );
        buf
    };
    let buf = draw(&mut table);
    assert_eq!(line(&buf, 1), "v0   v1   v2");
    table.move_col_by(4);
    let buf = draw(&mut table);
    assert_eq!(table.first_col(), 2);
    assert_eq!(line(&buf, 0), "C2   C3   C4");
    assert_eq!(line(&buf, 1), "v2   v3   v4");
    let theme = Theme::default();
    assert_eq!(
        buf[(10, 1)].bg,
        style(&theme, true, None).bg.unwrap(),
        "the cursor cell is the bright one"
    );
    assert_eq!(buf[(0, 1)].bg, active(&theme).bg.unwrap(), "the rest of its row the faint one");
}

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

#[test]
fn a_spreadsheet_numbers_its_rows_separates_its_columns_and_aligns_numbers_right() {
    let columns: Vec<Column> =
        (0..2).map(|_| Column::new("col", "", Width::Fit { max: 8 }).min(3)).collect();
    let mut table = Table::new().with_cells();
    let theme = Theme::default();
    let motion = crate::anim::Motion::new();
    let area = Rect::new(0, 0, 16, 2);
    let mut buf = Buffer::empty(area);
    let row = |i: usize| match i {
        0 => Row::new(vec!["name".into(), "qty".into()]),
        _ => Row::new(vec!["apple".into(), Cell::from("7").right()]),
    };
    table.view(2, &columns, row).numbered(true).separator(true).render(
        &Context::with_motion(&theme, &motion),
        area,
        &mut buf,
    );
    assert_eq!(line(&buf, 0), " 1 name  │ qty");
    assert_eq!(line(&buf, 1), " 2 apple │   7");
}
