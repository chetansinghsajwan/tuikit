//! The sensors pane: the battery and its history, the graphics card and every temperature, each a
//! bar, or `n/a` when the machine has no such sensor.

use super::super::{LABEL_W, Monitor};
use crate::format;
use crate::sample::Gpu;
use crate::sample::Temp;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ui::{Component, graph};

/// The most rows the battery history takes.
const HISTORY_ROWS: u16 = 4;

/// One meter row: `label`, a bar, and `value` (`n/a` and an empty track without a `ratio`).
fn meter_row(
    cx: &ui::Context,
    buf: &mut Buffer,
    area: Rect,
    (label, ratio, value): (&str, Option<f32>, Option<String>),
    reversed: bool,
) {
    graph::MeterRow {
        label,
        label_width: LABEL_W,
        ratio,
        value: value.as_deref().unwrap_or("n/a"),
        value_width: 4,
        reversed,
    }
    .render(cx, area, buf);
}

impl Monitor {
    /// The sensors pane: the battery (a bar, whether it charges, how long it lasts, its history),
    /// the graphics card, then every temperature as a bar toward its critical point.
    pub(in crate::monitor) fn draw_sensors(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let s = &self.sampler.snapshot;
        let inner = Self::pane(cx, buf, area, "sensors", false);
        if inner.height == 0 || inner.width == 0 {
            return;
        }
        let mut temps: Vec<Temp> = s.temps.clone();
        if let Some(celsius) = s.gpu.as_ref().and_then(|g| g.celsius) {
            temps.push(Temp { label: "gpu".into(), celsius, critical: None });
        }
        let below = 1 + 2 + 1 + temps.len().max(1) as u16;
        let mut y = self.draw_battery(buf, cx, inner, inner.y, below);
        y = Self::draw_gpu(buf, cx, inner, y, s.gpu.as_ref());
        if y >= inner.bottom() {
            return;
        }
        let row = |y| Rect { y, height: 1, ..inner };
        if temps.is_empty() {
            meter_row(cx, buf, row(y), ("temperature", None, None), false);
            return;
        }
        for (t, y) in temps.iter().zip(y..inner.bottom()) {
            let load = t.celsius / t.critical.filter(|c| *c > 0.0).unwrap_or(100.0);
            let value = format!("{:.0}°", t.celsius);
            meter_row(cx, buf, row(y), (&t.label, Some(load), Some(value)), false);
        }
    }

    /// The battery from row `y`: its bar, what it does, and (when `below` rows after it still fit)
    /// its history. Returns the row after the blank that follows.
    fn draw_battery(
        &self,
        buf: &mut Buffer,
        cx: &ui::Context,
        inner: Rect,
        y: u16,
        below: u16,
    ) -> u16 {
        let theme = cx.theme;
        let s = &self.sampler.snapshot;
        let muted = Style::new().fg(theme.colors.muted.into());
        let row = |y| Rect { y, height: 1, ..inner };
        let value = s.battery.map(|b| format!("{:.0}%", b.percent));
        let ratio = s.battery.map(|b| b.percent / 100.0);
        meter_row(cx, buf, row(y), ("battery", ratio, value), true);
        if y + 1 < inner.bottom() {
            let text = match s.battery {
                // Under a minute the estimate is noise (a battery at its end of charge).
                Some(b) => {
                    let left = b
                        .time_left
                        .filter(|t| *t >= 60)
                        .map(|t| format!("  {} left", format::duration(t)));
                    format!("{}{}", b.charge.label(), left.unwrap_or_default())
                }
                None => "no battery".to_string(),
            };
            buf.set_stringn(inner.x, y + 1, text, usize::from(inner.width), muted);
        }
        let mut next = y + 2;
        let rows = inner.bottom().saturating_sub(next + below).min(HISTORY_ROWS);
        if s.battery.is_some() && rows >= 2 {
            let area = Rect { y: next, height: rows, ..inner };
            let area = graph::axis(cx, buf, area, &[(1.0, "100%"), (0.5, "50%"), (0.0, "0")]);
            let history = self.sampler.battery_history.values();
            graph::history(buf, area, &history, |v| graph::heat(theme, 1.0 - v));
            next += rows;
        }
        // A blank row before what follows.
        next + 1
    }

    /// The graphics card from row `y`: how busy and how much of its memory is used, `n/a` without
    /// one. Returns the row after the blank that follows.
    fn draw_gpu(buf: &mut Buffer, cx: &ui::Context, inner: Rect, y: u16, gpu: Option<&Gpu>) -> u16 {
        let row = |y| Rect { y, height: 1, ..inner };
        let percent = |ratio: f32| format!("{:.0}%", ratio * 100.0);
        let load = gpu.map(|g| g.percent / 100.0);
        if y < inner.bottom() {
            meter_row(cx, buf, row(y), ("gpu", load, load.map(percent)), false);
        }
        let memory = gpu
            .filter(|g| g.memory_total > 0)
            .map(|g| g.memory_used as f32 / g.memory_total as f32);
        if y + 1 < inner.bottom() {
            meter_row(cx, buf, row(y + 1), ("gpu memory", memory, memory.map(percent)), false);
        }
        y + 3
    }
}
