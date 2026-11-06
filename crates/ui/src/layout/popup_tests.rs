use super::*;

#[test]
fn popup_is_centered_and_follows_the_percent_token() {
    let mut t = Theme::default();
    assert_eq!(t.layout.popup_percent, 90);
    let p = popup(&t, Rect::new(0, 0, 100, 50));
    assert_eq!((p.x, p.y, p.width, p.height), (5, 2, 90, 45));
    t.layout.popup_percent = 100;
    assert_eq!(popup(&t, Rect::new(0, 0, 100, 50)), Rect::new(0, 0, 100, 50));
    t.layout.popup_percent = 50;
    let q = popup(&t, Rect::new(4, 2, 101, 51));
    assert_eq!((q.width, q.height), (50, 25));
    assert!(q.x >= 4 && q.right() <= 105 && q.y >= 2 && q.bottom() <= 53);
    // Out-of-range values are clamped, never larger than the area.
    t.layout.popup_percent = 500;
    assert_eq!(popup(&t, Rect::new(0, 0, 30, 10)), Rect::new(0, 0, 30, 10));
}
