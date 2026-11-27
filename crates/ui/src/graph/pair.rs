//! Two histories in one graph (download over upload): both grow up from the bottom edge, each in its
//! own color, and where they overlap that part has a color of its own, so neither hides the other and
//! the overlap reads at every height.

use super::{filled_eighths, glyph};
use crate::theme::mix;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

/// How full the cell at `row` (0 at the bottom) is, in eighths, for a column filled to `eighths`.
fn cell_eighths(eighths: usize, row: u16) -> usize {
    eighths.saturating_sub(usize::from(row) * 8).min(8)
}

/// The color of where two series overlap: the blend of both, pushed toward `light` (the text color),
/// so it is told apart from either series alone and from the blend of the colors beside it.
pub fn overlap_color(a: Color, b: Color, light: Color) -> Color {
    mix(light, mix(b, a, 50), OVERLAP_LIGHT)
}

/// How much of `light` the overlap color takes (percent).
const OVERLAP_LIGHT: u8 = 40;

/// Draws `first` and `second` (each 0 to 1, oldest first, on the same scale) as columns in `area`,
/// the newest at the right edge, in `colors` (the first, the second, their overlap: see
/// [`overlap_color`]). The part of a column both reach is drawn in the overlap color and the part
/// only the taller reaches in its own, down to the eighth of a cell; so the shorter one is always
/// visible as where the overlap color ends. Values that do not fit are dropped from the left.
pub fn history_pair(
    buf: &mut Buffer,
    area: Rect,
    (first, second): (&[f32], &[f32]),
    (a, b, both): (Color, Color, Color),
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let width = usize::from(area.width);
    for i in 0..width {
        // The value of each series in column `i`, counted back from the right edge.
        let at = |values: &[f32]| (values.len() + i).checked_sub(width).map(|n| values[n]);
        let (ea, eb) = (
            at(first).map_or(0, |v| filled_eighths(v, area.height)),
            at(second).map_or(0, |v| filled_eighths(v, area.height)),
        );
        let x = area.x + i as u16;
        for row in 0..area.height {
            let (ha, hb) = (cell_eighths(ea, row), cell_eighths(eb, row));
            let y = area.bottom() - 1 - row;
            let cell = &mut buf[(x, y)];
            match (ha, hb) {
                (0, 0) => break,
                (h, 0) => {
                    cell.set_char(glyph(h)).set_style(Style::new().fg(a));
                }
                (0, h) => {
                    cell.set_char(glyph(h)).set_style(Style::new().fg(b));
                }
                (ha, hb) if ha.min(hb) == 8 => {
                    cell.set_char('█').set_style(Style::new().fg(both));
                }
                // One is full: its color is the part above the other, the overlap the part below.
                (ha, hb) if ha.max(hb) == 8 => {
                    let taller = if ha == 8 { a } else { b };
                    cell.set_char(glyph(ha.min(hb))).set_style(Style::new().fg(both).bg(taller));
                }
                // Both end inside this cell, so the cells below are all overlap. One glyph holds one
                // tone: it takes the overlap color when the shorter reaches over half the taller
                // (the overlap is most of what is drawn here), else the taller's own.
                (ha, hb) => {
                    let (lo, hi) = (ha.min(hb), ha.max(hb));
                    let taller = if ha > hb { a } else { b };
                    let tone = if lo * 2 >= hi { both } else { taller };
                    cell.set_char(glyph(hi)).set_style(Style::new().fg(tone));
                }
            }
        }
    }
}
