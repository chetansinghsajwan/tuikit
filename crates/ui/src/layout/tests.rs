use super::*;

#[test]
fn gap_changes_every_split() {
    let area = Rect::new(0, 0, 100, 10);
    let mut t = Theme::default();
    t.layout.gap = 0;
    let a = sidebar_main_preview(&t, area);
    assert_eq!(a[1].x, a[0].right());
    t.layout.gap = 2;
    let b = sidebar_main_preview(&t, area);
    assert_eq!(b[1].x, b[0].right() + 2);
    let c = main_preview(&t, area);
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
