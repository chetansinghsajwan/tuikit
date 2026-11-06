use super::*;
use ratatui::style::Style;

#[test]
fn colours_move_toward_the_background_and_default_ones_get_dim() {
    let mut theme = Theme::default();
    theme.colors.bg = crate::theme::Hex(Color::Rgb(0, 0, 0));
    let area = Rect::new(0, 0, 3, 1);
    let mut buf = Buffer::empty(area);
    buf.set_string(0, 0, "a", Style::new().fg(Color::Rgb(200, 100, 0)));
    buf.set_string(1, 0, "b", Style::new());
    dim(&mut buf, area, &theme);
    assert_eq!(buf[(0, 0)].fg, Color::Rgb(56, 28, 0));
    assert!(buf[(1, 0)].modifier.contains(Modifier::DIM));
}
