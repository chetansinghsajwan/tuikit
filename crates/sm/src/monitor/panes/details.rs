//! The details of one process, in a popup over the monitor.

use super::super::Monitor;
use crate::format;
use crate::sample::Details;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Widget};
use ui::{Pane, graph};

/// The widest the popup gets.
const MAX_WIDTH: u16 = 110;
/// The cells a popup's border and padding take across and down.
const FRAME_W: usize = 4;
const FRAME_H: usize = 2;
/// The cells a field's name takes before its value.
const NAME_W: usize = 10;

impl Monitor {
    /// Draws the details of process `pid` in a popup over `area`, dimming the monitor behind it.
    pub(in crate::monitor) fn draw_details(
        &self,
        pid: u32,
        area: Rect,
        buf: &mut Buffer,
        cx: &ui::Context,
    ) {
        let theme = cx.theme;
        let most = ui::layout::popup(cx, area);
        if most.width < 30 || most.height < 6 {
            return;
        }
        let muted = Style::new().fg(theme.colors.muted.into());
        // The popup is as wide as its longest value allows, and as tall as its lines.
        let width = most.width.min(MAX_WIDTH);
        let room = (usize::from(width) - FRAME_W, usize::from(most.height) - FRAME_H);
        let lines = match self.sampler.details(pid) {
            Some(d) => lines(cx, &d, room.0, room.1),
            None => vec![Line::from(Span::styled(format!("process {pid} has ended"), muted))],
        };
        let height = (lines.len() as u16 + FRAME_H as u16).min(most.height);
        let rect = Rect {
            x: area.x + (area.width - width) / 2,
            y: area.y + (area.height - height) / 2,
            width,
            height,
        };
        cx.motion.overlay(buf, area);
        ui::dim::dim(buf, area, cx);
        Clear.render(rect, buf);
        let inner = Pane::new().title("process").focused(true).draw(cx, rect, buf);
        Paragraph::new(lines).render(inner, buf);
    }
}

/// The lines of the popup: one per field, then the children as far as the height lets.
fn lines(cx: &ui::Context, d: &Details, width: usize, height: usize) -> Vec<Line<'static>> {
    let theme = cx.theme;
    let muted = Style::new().fg(theme.colors.muted.into());
    let field = |name: &str, value: Vec<Span<'static>>| {
        let mut spans = vec![Span::styled(format!("{name:<NAME_W$}"), muted)];
        spans.extend(value);
        Line::from(spans)
    };
    // A value that does not fit ends in `…`, never wraps into the next field.
    let text = |value: &str| vec![Span::raw(ui::list::fit(value, width.saturating_sub(NAME_W)))];
    let none = || vec![Span::styled("n/a", muted)];
    let optional = |value: &Option<String>| value.as_deref().map_or_else(none, text);
    let ratio = d.cpu / 100.0;
    let mut out = vec![
        Line::from(Span::styled(
            ui::list::fit(&d.name, width),
            Style::new().add_modifier(Modifier::BOLD),
        )),
        Line::default(),
        field("pid", text(&d.pid.to_string())),
        field("user", text(&d.user)),
        field("state", text(&d.status)),
        field(
            "parent",
            d.parent.as_ref().map_or_else(none, |(pid, name)| text(&format!("{pid}  {name}"))),
        ),
        field("running", text(&format::duration(d.run_time))),
        field("cpu", vec![graph::reading(theme, format!("{:.1}%", d.cpu), ratio)]),
        field(
            "memory",
            text(&format!(
                "{}  virtual {}",
                format::bytes(d.memory),
                format::bytes(d.virtual_memory)
            )),
        ),
        field("threads", d.threads.map_or_else(none, |n| text(&n.to_string()))),
        field(
            "disk",
            text(&format!(
                "read {}  written {}",
                format::bytes(d.read_total),
                format::bytes(d.written_total)
            )),
        ),
        field("program", optional(&d.exe)),
        field("folder", optional(&d.cwd)),
        field("command", if d.command.is_empty() { none() } else { text(&d.command) }),
    ];
    if !d.children.is_empty() && out.len() + 2 < height {
        out.push(Line::default());
        out.push(Line::from(Span::styled(format!("children {}", d.children.len()), muted)));
        let room = height - out.len();
        for (pid, name) in d.children.iter().take(room) {
            out.push(Line::from(vec![
                Span::styled(format!("{pid:>7}  "), muted),
                Span::raw(ui::list::fit(name, width.saturating_sub(9))),
            ]));
        }
    }
    out
}
