//! Tests for the theme-driven splits of `layout`.

use super::*;
use crate::Theme;

#[test]
fn gap_changes_every_split() {
    let area = Rect::new(0, 0, 100, 10);
    let mut t = Theme::default();
    t.layout.gap = 0;
    let a = sidebar_main_preview(&crate::Context::new(&t), area);
    assert_eq!(a[1].x, a[0].right());
    t.layout.gap = 2;
    let b = sidebar_main_preview(&crate::Context::new(&t), area);
    assert_eq!(b[1].x, b[0].right() + 2);
    let c = main_preview(&crate::Context::new(&t), area);
    assert_eq!(c[1].x, c[0].right() + 2);
}

#[test]
fn parses_json() {
    let t: Theme = serde_json::from_str(
        r##"{"chrome":{"border":"double"},"layout":{"gap":3},"colors":{"bg":"#102030"}}"##,
    )
    .unwrap();
    assert_eq!(t.layout.gap, 3);
    assert_eq!(t.chrome.border, "double");
}

#[test]
fn the_focused_pane_of_a_stack_takes_the_room() {
    let t = Theme::default();
    let cx = crate::Context::new(&t);
    let area = Rect::new(0, 0, 40, 30);
    let panes = focus_stack(&cx, area, 3, 1);
    assert_eq!(panes.iter().map(|r| r.height).collect::<Vec<_>>(), vec![6, 18, 6]);
    assert_eq!(panes[2].bottom(), area.bottom());
}
