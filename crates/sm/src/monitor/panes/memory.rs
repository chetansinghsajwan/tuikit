//! The memory pane: memory, swap and every disk, each a line of numbers over a bar.

use super::super::Monitor;
use super::super::glide::shown;
use crate::format;
use crate::sample::{Health, ratio};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ui::graph;

/// The most rows the memory history takes.
const HISTORY_ROWS: u16 = 4;

/// A labeled amount used out of a total, drawn as two rows: the numbers, then a bar filled to
/// `shown` (0 to 1), which glides to the true ratio.
struct Gauge<'a> {
    label: &'a str,
    /// What is used and the total, or `None` when the machine does not say (an empty bar and `n/a`).
    usage: Option<(u64, u64)>,
    shown: f32,
    /// More about it after the numbers (a disk's reads, writes and health).
    notes: Vec<Span<'static>>,
}

impl<'a> Gauge<'a> {
    fn new(label: &'a str, usage: Option<(u64, u64)>, shown: f32) -> Self {
        Gauge { label, usage, shown, notes: Vec::new() }
    }
}

impl Monitor {
    /// One labeled meter over two rows (the label with its numbers, then the bar); returns the row of
    /// the next meter, after a blank one.
    fn draw_meter(area: Rect, buf: &mut Buffer, cx: &ui::Context, y: u16, gauge: Gauge) -> u16 {
        let theme = cx.theme;
        let Gauge { label, usage, shown, notes } = gauge;
        if y + 1 >= area.bottom() {
            return area.bottom();
        }
        let muted = Style::new().fg(theme.colors.muted.into());
        let mut spans = vec![Span::raw(label.to_string())];
        match usage {
            Some((used, total)) => {
                let r = ratio(used, total);
                spans.push(Span::styled(
                    format!("  {} / {}  ", format::bytes(used), format::bytes(total)),
                    muted,
                ));
                spans.push(graph::reading(theme, format!("{:.0}%", r * 100.0), r));
            }
            None => spans.push(Span::styled("  n/a", muted)),
        }
        spans.extend(notes);
        let row = |y| Rect { y, height: 1, ..area };
        Paragraph::new(Line::from(spans)).render(row(y), buf);
        graph::meter(
            buf,
            row(y + 1),
            if usage.is_some() { shown } else { 0.0 },
            |place| graph::heat(theme, place),
            theme.colors.border.into(),
        );
        // A blank row before the next meter.
        y + 3
    }

    /// The memory pane: memory, swap and every disk.
    pub(in crate::monitor) fn draw_memory(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let s = &self.sampler.snapshot;
        let inner = Self::pane(cx, buf, area, "memory", false);
        if inner.height == 0 || inner.width == 0 {
            return;
        }
        let now = self.frame_time;
        let mem_ratio = ratio(s.memory_used, s.memory_total);
        let swap_ratio = ratio(s.swap_used, s.swap_total);
        let memory = self.glide.memory.map_or(mem_ratio, |bar| bar.value(now));
        let swap = self.glide.swap.map_or(swap_ratio, |bar| bar.value(now));
        let mut y = inner.y;
        let gauge = Gauge::new("mem", Some((s.memory_used, s.memory_total)), memory);
        y = Self::draw_meter(inner, buf, cx, y, gauge);
        y = self.draw_memory_history(inner, buf, cx, y);
        // A machine without swap has none to show: the pane says so.
        let usage = (s.swap_total > 0).then_some((s.swap_used, s.swap_total));
        y = Self::draw_meter(inner, buf, cx, y, Gauge::new("swap", usage, swap));
        let c = &cx.theme.colors;
        let muted = Style::new().fg(c.muted.into());
        if s.disks.is_empty() {
            Self::draw_meter(inner, buf, cx, y, Gauge::new("disks", None, 0.0));
            return;
        }
        for (i, disk) in s.disks.iter().enumerate() {
            let fallback = ratio(disk.used, disk.total);
            let usage = Some((disk.used, disk.total));
            let mut gauge =
                Gauge::new(&disk.mount, usage, shown(&self.glide.disks, i, now, fallback));
            gauge.notes.push(Span::styled(
                format!("  r {}  w {}", format::rate(disk.read), format::rate(disk.written)),
                muted,
            ));
            match disk.health {
                Some(Health::Good) => gauge.notes.push(Span::styled("  healthy", muted)),
                Some(Health::Failing) => gauge.notes.push(Span::styled(
                    "  failing",
                    Style::new().fg(c.error.into()).add_modifier(Modifier::BOLD),
                )),
                None => gauge.notes.push(Span::styled("  health n/a", muted)),
            }
            y = Self::draw_meter(inner, buf, cx, y, gauge);
        }
    }

    /// The history of memory use as a graph under its meter, when the pane is tall enough to keep
    /// the swap meter below it (a graph under two rows is not worth the room); returns the row after
    /// it.
    fn draw_memory_history(&self, inner: Rect, buf: &mut Buffer, cx: &ui::Context, y: u16) -> u16 {
        let theme = cx.theme;
        let swap_rows = 3;
        let rows = inner.bottom().saturating_sub(y + swap_rows).min(HISTORY_ROWS);
        if rows < 2 {
            return y;
        }
        let total = self.sampler.snapshot.memory_total;
        let (high, middle) = (format::bytes(total), format::bytes(total / 2));
        let area = Rect { y, height: rows, ..inner };
        let area = graph::axis(cx, buf, area, &[(1.0, &high), (0.5, &middle), (0.0, "0")]);
        let history = self.sampler.memory_history.values();
        graph::history(buf, area, &history, |v| graph::heat(theme, v));
        y + rows + 1
    }
}
