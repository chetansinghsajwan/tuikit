//! The network pane: download and upload in one graph, both on the same scale, the overlap in a color
//! of its own.

use super::super::Monitor;
use crate::format;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ui::graph;

impl Monitor {
    /// The network pane: the two rates (`▼ download  ▲ upload`), then one graph of both with its
    /// scale; where they overlap their colors blend. The top of the graph is a round step (5K/s,
    /// 10K/s, 50K/s, ... 1G/s) that holds what the graph shows.
    pub(in crate::monitor) fn draw_network(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let theme = cx.theme;
        let s = &self.sampler.snapshot;
        let inner = Self::pane(cx, buf, area, "network", false);
        if inner.height < 2 || inner.width == 0 {
            return;
        }
        let (down, up) = (theme.colors.info.into(), theme.colors.accent.into());
        let both = graph::overlap_color(down, up, theme.colors.fg.into());
        let facts = vec![
            Span::styled(format!("▼ {}", format::rate(s.download)), Style::new().fg(down)),
            Span::raw("  "),
            Span::styled(format!("▲ {}", format::rate(s.upload)), Style::new().fg(up)),
            Span::raw("  "),
            Span::styled("█ both", Style::new().fg(both)),
        ];
        let rest = Self::info(buf, inner, facts);
        let (download, upload) = (&self.sampler.download_history, &self.sampler.upload_history);
        let shown = usize::from(rest.width);
        let seen = download.peak_of_last(shown, 0.0).max(upload.peak_of_last(shown, 0.0));
        let top = graph::step_up(&format::RATE_STEPS, seen);
        let (high, middle) = (format::rate_label(top), format::rate_label(top / 2.0));
        let area = graph::axis(cx, buf, rest, &[(1.0, &high), (0.5, &middle), (0.0, "0")]);
        graph::history_pair(
            buf,
            area,
            (&download.scaled_to(top), &upload.scaled_to(top)),
            (down, up, both),
        );
    }
}
