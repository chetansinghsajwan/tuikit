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
