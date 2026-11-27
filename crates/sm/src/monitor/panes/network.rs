//! The network pane: download and upload in one graph, both on the same scale.

use super::super::Monitor;
use crate::format;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ui::graph;

/// What a graph's top stands for at least, in bytes per second, so a quiet network does not draw
/// its noise at full height.
const FLOOR: f32 = 4096.0;

impl Monitor {
    /// The network pane: the two rates (`▼ download  ▲ upload`), then one graph of both with its
    /// scale; where they overlap their colors blend.
    pub(in crate::monitor) fn draw_network(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let theme = cx.theme;
        let s = &self.sampler.snapshot;
        let inner = Self::pane(cx, buf, area, "network", false);
        if inner.height < 2 || inner.width == 0 {
            return;
        }
        let (down, up) = (theme.colors.info.into(), theme.colors.accent.into());
        let facts = vec![
            Span::styled(format!("▼ {}", format::rate(s.download)), Style::new().fg(down)),
            Span::raw("  "),
            Span::styled(format!("▲ {}", format::rate(s.upload)), Style::new().fg(up)),
        ];
        let rest = Self::info(buf, inner, facts);
        let (download, upload) = (&self.sampler.download_history, &self.sampler.upload_history);
        let peak = download.peak(FLOOR).max(upload.peak(FLOOR));
        let area = graph::axis(cx, buf, rest, &format::rate(peak as u64), "0");
        graph::history_pair(
            buf,
            area,
            (&download.scaled_to(peak), &upload.scaled_to(peak)),
            (down, up),
        );
    }
}
