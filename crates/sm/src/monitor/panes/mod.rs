//! The panes of the monitor, one file each; this holds what they share. A pane's title is only its
//! name: what else it has to say (the load, the count, the filter) is its first line inside.

/// The CPU pane.
mod cpu;
/// The details popup of one process.
mod details;
/// The memory pane.
mod memory;
/// The network pane.
mod network;
/// The process pane.
mod processes;
/// The sensors pane.
mod sensors;

pub(super) use processes::columns as process_columns;

use super::Monitor;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ui::Pane;

impl Monitor {
    /// A pane titled `title` over `area`, returning its inner area (the pane is drawn).
    fn pane(cx: &ui::Context, buf: &mut Buffer, area: Rect, title: &str, focused: bool) -> Rect {
        let theme = cx.theme;
        let mut pane = Pane::new().title(title).focused(focused);
        if let Some(icon) = theme.icons.title(title) {
            pane = pane.icon(icon);
        }
        pane.draw(cx, area, buf)
    }

    /// The first line inside a pane, `spans` on it; returns what is left of `inner` below it.
    fn info(buf: &mut Buffer, inner: Rect, spans: Vec<Span<'static>>) -> Rect {
        Paragraph::new(Line::from(spans)).render(Rect { height: 1, ..inner }, buf);
        Rect { y: inner.y + 1, height: inner.height.saturating_sub(1), ..inner }
    }
}
