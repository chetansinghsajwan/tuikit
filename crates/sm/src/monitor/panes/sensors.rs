//! The sensors pane: the battery and every temperature, each a bar, or `n/a` when the machine has
//! no such sensor.

use super::super::{LABEL_W, Monitor};
use crate::format;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ui::{Component, graph};

impl Monitor {
    /// The sensors pane: the battery (a bar, whether it charges, how long it lasts), then every
    /// temperature as a bar toward its critical point.
    pub(in crate::monitor) fn draw_sensors(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let s = &self.sampler.snapshot;
        let inner = Self::pane(cx, buf, area, "sensors", false);
        if inner.height == 0 || inner.width == 0 {
            return;
        }
        let muted = Style::new().fg(cx.theme.colors.muted.into());
        let row = |y| Rect { y, height: 1, ..inner };
        let mut y = inner.y;
        let value = s.battery.map(|b| format!("{:.0}%", b.percent));
        graph::MeterRow {
            label: "battery",
            label_width: LABEL_W,
            ratio: s.battery.map(|b| b.percent / 100.0),
            value: value.as_deref().unwrap_or("n/a"),
            value_width: 4,
            reversed: true,
        }
        .render(cx, row(y), buf);
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
        // A blank row before the temperatures.
        y += 3;
        if s.temps.is_empty() {
            if y < inner.bottom() {
                graph::MeterRow {
                    label: "temperature",
                    label_width: LABEL_W,
                    ratio: None,
                    value: "n/a",
                    value_width: 4,
                    reversed: false,
                }
                .render(cx, row(y), buf);
            }
            return;
        }
        for t in &s.temps {
            if y >= inner.bottom() {
                break;
            }
            let value = format!("{:.0}°", t.celsius);
            graph::MeterRow {
                label: &t.label,
                label_width: LABEL_W,
                ratio: Some(t.celsius / t.critical.filter(|c| *c > 0.0).unwrap_or(100.0)),
                value: &value,
                value_width: 4,
                reversed: false,
            }
            .render(cx, row(y), buf);
            y += 1;
        }
    }
}
