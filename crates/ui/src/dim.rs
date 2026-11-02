//! Dim what is behind a popup, so the popup stands out. A terminal cannot blur, so colours are
//! mixed toward the background colour; cells whose colour is the terminal's own default get the
//! terminal's "dim" attribute instead.

use crate::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

/// How much of the original colour is left (percent).
pub const KEEP: u8 = 45;

/// Dim every cell of `area`.
pub fn dim(buf: &mut Buffer, area: Rect, theme: &Theme) {
    let base: Color = theme.colors.bg.into();
    let area = area.intersection(buf.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            match cell.fg {
                Color::Rgb(..) => cell.fg = crate::mix(cell.fg, base, KEEP),
                _ => {
                    cell.modifier.insert(Modifier::DIM);
                }
            }
            if matches!(cell.bg, Color::Rgb(..)) {
                cell.bg = crate::mix(cell.bg, base, KEEP);
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(buf[(0, 0)].fg, Color::Rgb(90, 45, 0));
        assert!(buf[(1, 0)].modifier.contains(Modifier::DIM));
    }
}
