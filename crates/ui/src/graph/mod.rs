//! Graphs and meters: the one place that draws a history of values as bars and a ratio as a filled
//! bar, so every tool that shows a load (CPU, memory, a transfer rate) draws it the same way.

/// The scale at the left edge of a graph.
mod axis;
/// Two histories drawn as one graph.
mod pair;
/// Round values for the top of a graph.
mod scale;

pub use axis::axis;
pub use pair::{history_pair, overlap_color};
pub use scale::step_up;

use crate::anim;
use crate::component::{Component, Context};
use crate::theme::{Theme, mix};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use std::time::{Duration, Instant};

/// How long a bar takes to glide to a new value.
const GLIDE: Duration = Duration::from_millis(400);

/// The heat color of a load between 0 and 1: green (`success`) through yellow (`warning`) to red
/// (`error`), blended smoothly, so a bar or graph shows how hot it is by its color alone. Without
/// truecolor the theme's colors are not blendable and the nearer of the three is used.
pub fn heat(theme: &Theme, load: f32) -> Color {
    let c = &theme.colors;
    let load = if load.is_finite() { load.clamp(0.0, 1.0) } else { 0.0 };
    let (from, to, share) = if load < 0.5 {
        (c.success.into(), c.warning.into(), load * 2.0)
    } else {
        (c.warning.into(), c.error.into(), (load - 0.5) * 2.0)
    };
    match mix(to, from, (share * 100.0).round() as u8) {
        blended @ Color::Rgb(..) => blended,
        _ if share < 0.5 => from,
        _ => to,
    }
}

/// `text` as a span in the heat color of `load` (0 to 1): a number that measures a load, colored by
/// how hot it is like the bar beside it.
pub fn reading(theme: &Theme, text: impl Into<String>, load: f32) -> Span<'static> {
    Span::styled(text.into(), Style::new().fg(heat(theme, load)))
}

/// A value that glides to each new target instead of jumping, for bars that are fed a reading now and
/// then. The tool sets the target when a reading arrives and draws [`Eased::value`] each frame; while
/// [`Eased::is_moving`] it asks for the next frame with `Motion::request_frame`.
#[derive(Debug, Clone, Copy)]
pub struct Eased {
    from: f32,
    to: f32,
    since: Instant,
}

impl Eased {
    /// A value at rest at `value`.
    pub fn new(value: f32) -> Eased {
        Eased { from: value, to: value, since: Instant::now() }
    }

    /// Glides from where the value is at `now` to `target`; the same target again changes nothing.
    pub fn set(&mut self, target: f32, now: Instant) {
        if target == self.to {
            return;
        }
        self.from = self.value(now);
        self.to = target;
        self.since = now;
    }

    /// The value at `now`: the target itself when motion is off.
    pub fn value(&self, now: Instant) -> f32 {
        if !self.is_moving(now) {
            return self.to;
        }
        let t = now.saturating_duration_since(self.since).as_secs_f32()
            / anim::scaled(GLIDE).as_secs_f32();
        self.from + (self.to - self.from) * anim::ease_out(t)
    }

    /// Whether the value is still on its way at `now`.
    pub fn is_moving(&self, now: Instant) -> bool {
        anim::is_on()
            && self.from != self.to
            && now.saturating_duration_since(self.since) < anim::scaled(GLIDE)
    }
}

/// The block glyphs from one eighth of a cell up to a full cell.
const EIGHTHS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// The glyph of a column that is `eighths` eighths of a cell full (1 to 8).
fn glyph(eighths: usize) -> char {
    EIGHTHS[eighths.clamp(1, 8) - 1]
}

/// How many eighths of the `height` rows a `value` between 0 and 1 fills; a value above 0 always
/// shows at least one eighth, so a small load is not mistaken for none.
pub fn filled_eighths(value: f32, height: u16) -> usize {
    let value = if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 };
    let total = usize::from(height) * 8;
    let filled = (value * total as f32).round() as usize;
    if value > 0.0 { filled.max(1) } else { 0 }
}

/// Draws `values` (each 0 to 1, oldest first) as columns in `area`, the newest at the right edge.
/// Older values that do not fit are dropped; when there are fewer values than columns the left
/// part stays empty. `color` gives the color of a cell from the level it reaches in the graph, 0 at
/// the bottom edge to 1 at the top (`heat` makes every column the same gradient, green at the foot
/// to red at the top, cut where its value ends; a constant gives a flat color).
pub fn history(buf: &mut Buffer, area: Rect, values: &[f32], color: impl Fn(f32) -> Color) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let shown = &values[values.len().saturating_sub(usize::from(area.width))..];
    let first = area.right() - shown.len() as u16;
    for (i, value) in shown.iter().enumerate() {
        let x = first + i as u16;
        let mut left = filled_eighths(*value, area.height);
        for row in 0..area.height {
            let y = area.bottom() - 1 - row;
            let here = left.min(8);
            if here == 0 {
                break;
            }
            let level =
                (usize::from(row) * 8 + here) as f32 / (usize::from(area.height) * 8) as f32;
            buf[(x, y)].set_char(glyph(here)).set_style(Style::new().fg(color(level)));
            left -= here;
        }
    }
}

/// Draws a bar of `ratio` (0 to 1) across one row of `area`: a thin horizontal line, heavy where it is
/// filled (each cell colored by `color` from its place along the bar, 0 at the left edge and 1 at the
/// right: `heat` makes it a gradient, in half cells), the rest a light line in `track`. The bar only
/// grows sideways; it never fills a cell upward.
pub fn meter(buf: &mut Buffer, area: Rect, ratio: f32, color: impl Fn(f32) -> Color, track: Color) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let ratio = if ratio.is_finite() { ratio.clamp(0.0, 1.0) } else { 0.0 };
    let halves = (ratio * f32::from(area.width) * 2.0).round() as usize;
    let halves = if ratio > 0.0 { halves.max(1) } else { 0 };
    for i in 0..area.width {
        let here = halves.saturating_sub(usize::from(i) * 2).min(2);
        let cell = &mut buf[(area.x + i, area.y)];
        if here == 0 {
            cell.set_char('─').set_style(Style::new().fg(track));
        } else {
            let place = (f32::from(i) + 0.5) / f32::from(area.width);
            let glyph = if here == 2 { '━' } else { '╸' };
            cell.set_char(glyph).set_style(Style::new().fg(color(place)));
        }
    }
}

/// One meter on one row: the label (muted), a bar filled to `ratio` in the heat gradient, then the
/// value in the color of the load, with one blank cell between the parts so they never touch. The
/// label and the value take `label_width` and `value_width` cells, so the bars of a column of rows
/// line up; the bar takes the rest. A label longer than its width ends in `…`. With `reversed` a
/// high value is the good one (a battery): the gradient and the value's color run the other way. A
/// `ratio` of `None` is a reading that is not available: an empty track, and `value` muted.
pub struct MeterRow<'a> {
    pub label: &'a str,
    pub label_width: usize,
    pub ratio: Option<f32>,
    pub value: &'a str,
    pub value_width: usize,
    pub reversed: bool,
}

impl Component for MeterRow<'_> {
    fn render(self, cx: &Context, area: Rect, buf: &mut Buffer) {
        let theme = cx.theme;
        let fixed = (self.label_width + 1 + 1 + self.value_width) as u16;
        if area.height == 0 || area.width <= fixed {
            return;
        }
        let muted = Style::new().fg(theme.colors.muted.into());
        let label = crate::list::fit(self.label, self.label_width);
        buf.set_stringn(area.x, area.y, label, self.label_width, muted);
        let tone = |v: f32| heat(theme, if self.reversed { 1.0 - v } else { v });
        let bar = Rect {
            x: area.x + self.label_width as u16 + 1,
            width: area.width - fixed,
            height: 1,
            ..area
        };
        let ratio = self.ratio.unwrap_or(0.0);
        meter(buf, bar, ratio, tone, theme.colors.border.into());
        let value = format!("{:>width$}", self.value, width = self.value_width);
        let color = match self.ratio {
            Some(ratio) => Style::new().fg(tone(ratio)),
            None => muted,
        };
        buf.set_stringn(bar.right() + 1, area.y, value, self.value_width, color);
    }
}

/// Tests for the graph and the meter.
#[cfg(test)]
mod tests;
