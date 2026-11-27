//! The scale of a graph: what its top and its bottom stand for, in the column at its left edge.

use crate::component::Context;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::UnicodeWidthStr;

/// Draws `top` (what the full height stands for, `100%` or `1.2M/s`) at the top left of `area` and
/// `bottom` (`0`) at the bottom left, muted and right aligned to each other, and returns the area
/// right of them, one blank cell away, for the graph. A graph that is one row tall shows only `top`;
/// an area too narrow to leave the graph room is returned whole, without a scale.
pub fn axis(cx: &Context, buf: &mut Buffer, area: Rect, top: &str, bottom: &str) -> Rect {
    let width = top.width().max(bottom.width()) as u16;
    if area.height == 0 || area.width < width + 3 {
        return area;
    }
    let muted = Style::new().fg(cx.theme.colors.muted.into());
    let put = |buf: &mut Buffer, y: u16, text: &str| {
        let pad = width - text.width() as u16;
        buf.set_stringn(area.x + pad, y, text, usize::from(width), muted);
    };
    put(buf, area.y, top);
    if area.height > 1 {
        put(buf, area.bottom() - 1, bottom);
    }
    Rect { x: area.x + width + 1, width: area.width - width - 1, ..area }
}
