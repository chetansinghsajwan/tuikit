//! The CPU pane: how the machine is doing now, its load over time, and every core.

use super::super::glide::shown;
use super::super::{GAP, Monitor};
use crate::format;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ui::{Component, graph};

/// The most rows the history graph takes; the cores under it keep what they need first.
const GRAPH_ROWS: u16 = 5;

impl Monitor {
    /// The CPU pane: a line of facts, the load of the whole machine over time as a graph, then the
    /// cores in two columns as horizontal bars.
    pub(in crate::monitor) fn draw_cpu(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let theme = cx.theme;
        let s = &self.sampler.snapshot;
        let inner = Self::pane(cx, buf, area, "cpu", false);
        if inner.height == 0 || inner.width == 0 {
            return;
        }
        let muted = Style::new().fg(theme.colors.muted.into());
        let facts = vec![
            Span::styled(
                format!("{:.0}%", s.cpu),
                Style::new().fg(graph::heat(theme, s.cpu / 100.0)),
            ),
            Span::styled(
                format!(
                    "  load {:.2} {:.2} {:.2}  up {}",
                    s.load.0,
                    s.load.1,
                    s.load.2,
                    format::duration(s.uptime)
                ),
                muted,
            ),
        ];
        let rest = Self::info(buf, inner, facts);
        if rest.height == 0 {
            return;
        }
        let core_rows = (s.cores.len() as u16).div_ceil(2);
        let graph_rows = rest.height.saturating_sub(core_rows + 1).clamp(1, GRAPH_ROWS);
        let area = Rect { height: graph_rows.min(rest.height), ..rest };
        let area = graph::axis(cx, buf, area, &[(1.0, "100%"), (0.5, "50%"), (0.0, "0")]);
        let history = self.sampler.cpu_history.values();
        graph::history(buf, area, &history, |v| graph::heat(theme, v));

        let top = rest.y + graph_rows + 1;
        let core_rows = core_rows.min(rest.bottom().saturating_sub(top));
        if core_rows == 0 {
            return;
        }
        // A gap of two cells between the columns keeps a value off the next label.
        let half = inner.width.saturating_sub(GAP) / 2;
        for (i, load) in s.cores.iter().enumerate() {
            let row = i as u16 % core_rows;
            let column = i as u16 / core_rows;
            if column > 1 {
                break;
            }
            let cell = Rect {
                x: inner.x + column * (half + GAP),
                y: top + row,
                width: if column == 0 { half } else { inner.width - half - GAP },
                height: 1,
            };
            let ratio = shown(&self.glide.cores, i, self.frame_time, load / 100.0);
            Self::draw_load(cx, buf, cell, &i.to_string(), ratio);
        }
    }

    /// One load on one row: its label, a bar and its percent.
    fn draw_load(cx: &ui::Context, buf: &mut Buffer, area: Rect, label: &str, ratio: f32) {
        let value = format!("{:.0}%", ratio * 100.0);
        graph::MeterRow {
            label,
            label_width: 3,
            ratio: Some(ratio),
            value: &value,
            value_width: 4,
            reversed: false,
        }
        .render(cx, area, buf);
    }
}
