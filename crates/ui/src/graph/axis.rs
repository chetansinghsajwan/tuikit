//! The scale of a graph: what its top, its middle and its bottom stand for, in the column at its left
//! edge.

use crate::component::Context;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::UnicodeWidthStr;

/// Draws `ticks` (a place from 1 at the top to 0 at the bottom, and its label: `(1.0, "10M/s")`,
/// `(0.5, "5M/s")`, `(0.0, "0")`) at the left of `area`, muted and right aligned to each other, and
/// returns the area right of them, one blank cell away, for the graph. A tick whose row is taken by
/// an earlier one is left out (a short graph keeps its top and bottom), and a graph one row tall
/// shows only the first. An area too narrow to leave the graph room is returned whole, without a
/// scale.
pub fn axis(cx: &Context, buf: &mut Buffer, area: Rect, ticks: &[(f32, &str)]) -> Rect {
    let width = ticks.iter().map(|(_, label)| label.width()).max().unwrap_or(0) as u16;
    if area.height == 0 || area.width < width + 3 {
        return area;
    }
    let muted = Style::new().fg(cx.theme.colors.muted.into());
    let last = f32::from(area.height - 1);
    let mut taken = Vec::new();
    for (place, label) in ticks {
        let row = ((1.0 - place.clamp(0.0, 1.0)) * last).round() as u16;
        if taken.contains(&row) {
            continue;
        }
        taken.push(row);
        let pad = width - label.width() as u16;
        buf.set_stringn(area.x + pad, area.y + row, label, usize::from(width), muted);
    }
    Rect { x: area.x + width + 1, width: area.width - width - 1, ..area }
}
