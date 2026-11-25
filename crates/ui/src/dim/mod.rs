//! Dim what is behind a popup, so the popup stands out. A terminal cannot blur, so colours are
//! mixed toward the background colour; cells whose colour is the terminal's own default get the
//! terminal's "dim" attribute instead.

use crate::component::Context;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

/// How much of the original colour is left behind a popup (percent).
pub const KEEP: u8 = 28;
/// How much is left in a pane that does not have the focus (a lighter dimming).
pub const KEEP_INACTIVE: u8 = 62;

/// Dim every cell of `area`.
pub fn dim(buf: &mut Buffer, area: Rect, cx: &Context) {
    dim_to(buf, area, cx, KEEP);
}

/// Dim every cell of `area`, keeping `keep` percent of each colour.
pub fn dim_to(buf: &mut Buffer, area: Rect, cx: &Context, keep: u8) {
    let theme = cx.theme;
    let base: Color = theme.colors.bg.into();
    let area = area.intersection(buf.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            match cell.fg {
                Color::Rgb(..) => cell.fg = crate::mix(cell.fg, base, keep),
                _ => {
                    cell.modifier.insert(Modifier::DIM);
                }
            }
            if matches!(cell.bg, Color::Rgb(..)) {
                cell.bg = crate::mix(cell.bg, base, keep);
            }
        }
    }
}

#[cfg(test)]
mod tests;
