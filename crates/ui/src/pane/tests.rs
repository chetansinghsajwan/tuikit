use super::*;
use ratatui::style::Color;

fn bg_at(theme: &Theme, x: u16, y: u16) -> Color {
    let area = Rect::new(0, 0, 10, 5);
    let mut buf = Buffer::empty(area);
    Pane::new(theme).title("t").render(area, &mut buf);
    buf[(x, y)].bg
}

#[test]
fn transparent_panes_leave_the_terminal_background_alone() {
    let mut t = Theme::default();
    assert!(t.chrome.transparent, "transparent by default");
    assert_eq!(bg_at(&t, 3, 2), Color::Reset);
    t.chrome.transparent = false;
    assert_eq!(bg_at(&t, 3, 2), Color::from(t.colors.bg));
}
