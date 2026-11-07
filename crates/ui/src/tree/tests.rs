use super::*;

fn text(spans: &[Span]) -> String {
    spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn a_dir_has_guides_and_a_folder_icon_a_group_has_an_arrow() {
    let theme = Theme::load();
    let d = text(&dir(&theme, 2, "src", true));
    assert!(d.starts_with("│ │ ") && d.ends_with("src") && !d.contains('▾'), "{d}");
    assert!(text(&group(&theme, 1, true, None, "Tags", Style::new())).starts_with("│ ▸ Tags"));
    assert!(text(&group(&theme, 0, false, None, "Tags", Style::new())).starts_with("▾ Tags"));
    assert_eq!(text(&[count(&theme, 3)]), " 3");
}

fn row_text(row: &FileRow, width: u16) -> String {
    let theme = Theme::load();
    let area = Rect::new(0, 0, width, 1);
    let mut buf = Buffer::empty(area);
    file_row(&theme, &mut buf, &list::Rows::new(area, area), 0, row);
    (0..width).map(|x| buf[(x, 0)].symbol().to_string()).collect()
}

#[test]
fn a_long_name_shrinks_and_the_rest_of_the_row_keeps_its_place() {
    let long = FileRow {
        name: "a-very-long-file-name-that-does-not-fit.rs",
        count: Some(7),
        trailing: vec![Span::raw("+3 -1 ")],
        signs: vec![('M', Color::Red)],
        ..Default::default()
    };
    let t = row_text(&long, 30);
    assert_eq!(t.chars().count(), 30);
    assert!(t.contains('…') && t.trim_end().ends_with("+3 -1 M"), "{t}");
    // A pane too narrow for the trailing text drops it and keeps the name.
    let t = row_text(
        &FileRow {
            trailing: vec![Span::raw("a-very-wide-column")],
            ..FileRow { name: "main.rs", ..Default::default() }
        },
        20,
    );
    assert!(t.contains("main.rs") && !t.contains("column"), "{t}");
    // Wide characters are counted by cells, so the sign stays in the last cell.
    let t = row_text(
        &FileRow { name: "日本語日本語日本語日本語", signs: vec![('M', Color::Red)], ..Default::default() },
        16,
    );
    assert!(t.trim_end().ends_with('M') && t.contains('…'), "{t}");
}

#[test]
fn the_scope_is_the_folder_under_the_cursor_or_the_one_holding_it() {
    // a / a/b / a/b/x / a/y / z
    let depths = [0, 1, 2, 1, 0];
    assert_eq!(active_scope(&depths, 1), Some(Scope { from: 1, to: 3, col: 1 }), "an open folder: its contents");
    assert_eq!(active_scope(&depths, 2), Some(Scope { from: 1, to: 3, col: 1 }), "a file: the folder that holds it");
    assert_eq!(active_scope(&depths, 3), Some(Scope { from: 0, to: 4, col: 0 }));
    assert_eq!(active_scope(&depths, 4), None, "a top-level file has no scope");
    assert_eq!(active_scope(&depths, 0).and_then(|s| s.col_at(2)), Some(0));
    assert_eq!(active_scope(&depths, 0).and_then(|s| s.col_at(4)), None);
}

#[test]
fn indent_guides_replace_the_leading_spaces_at_each_unit() {
    let theme = Theme::load();
    let text = |s: &[Span]| -> String { s.iter().map(|s| s.content.as_ref()).collect() };
    let line = vec![Span::raw("        "), Span::raw("let x = 1;")];
    assert_eq!(text(&mark_indent(&theme, line, 8, 4)), "│   │   let x = 1;");
    // Only up to the indentation given (a blank line borrows its neighbors'), and never inside the code.
    assert_eq!(text(&mark_indent(&theme, vec![Span::raw("      a  b")], 4, 2)), "│ │   a  b");
    assert_eq!(text(&mark_indent(&theme, vec![Span::raw("    ")], 0, 4)), "    ");
    // The spaces of one span split across highlight pieces still count as one column run.
    assert_eq!(text(&mark_indent(&theme, vec![Span::raw("  "), Span::raw("  "), Span::raw("x")], 4, 2)), "│ │ x");
}

#[test]
fn line_counts_line_up_in_columns() {
    let theme = Theme::load();
    let text = |s: &[Span]| -> String { s.iter().map(|s| s.content.as_ref()).collect() };
    let changes = [Some((3, 120)), Some((1204, 4)), None];
    let w = CountWidths::of(changes);
    let rows: Vec<String> = changes.iter().map(|c| text(&line_counts(&theme, *c, w))).collect();
    assert!(rows.iter().all(|r| r.chars().count() == w.width()), "{rows:?}");
    assert_eq!(rows[0], "   +3 −120", "{rows:?}");
    assert_eq!(rows[1], "+1204 −4  ", "{rows:?}");
    assert_eq!(rows[2], "    binary", "{rows:?}");
}

#[test]
fn the_active_guide_is_drawn_in_the_accent() {
    let theme = Theme::load();
    let text = |s: &[Span]| -> String { s.iter().map(|s| s.content.as_ref()).collect() };
    let marked = guides_marked(&theme, 3, Some(1));
    assert_eq!(text(&marked), guides(&theme, 3).content);
    let accent: Color = theme.colors.accent.into();
    assert_eq!(
        marked.iter().filter(|s| s.style.fg == Some(accent)).map(|s| s.content.as_ref()).collect::<String>(),
        "│ "
    );
    // A column that is not among the guides marks nothing.
    assert_eq!(guides_marked(&theme, 1, Some(1)).len(), 1);
}

#[test]
fn a_file_row_shows_only_the_options_asked_for() {
    let plain = FileRow { name: "main.rs", ..Default::default() };
    let t = row_text(&plain, 40);
    assert!(t.contains("main.rs") && !t.contains('+') && t.trim_end().ends_with("main.rs"), "{t}");

    let dir = FileRow { name: "src", is_dir: true, count: Some(4), ..Default::default() };
    assert!(row_text(&dir, 40).contains("src 4"));

    let full = FileRow {
        name: "main.rs",
        trailing: vec![Span::raw("+3 -1")],
        signs: vec![('M', Color::Red)],
        ..Default::default()
    };
    let t = row_text(&full, 40);
    assert!(t.trim_end().ends_with("+3 -1M"), "trailing text ends just left of the sign cell: {t}");
}
