//! Tests for `Context` and the components drawn through it.

use super::*;
use crate::Pane;
use crate::list::{ListRow, Rows};
use ratatui::text::Span;
use std::time::Duration;

/// Draws one row into a list of `area` and finishes the frame with `cx`.
fn frame(cx: &Context, area: Rect, rows_at: &[(u16, &'static str)]) -> Buffer {
    let mut buf = Buffer::empty(area);
    let rows = Rows::new(area, area);
    for &(y, text) in rows_at {
        let row = ListRow { left: vec![Span::raw(text)], ..Default::default() };
        row.render(cx, &mut buf, &rows, y);
    }
    cx.motion.finish(&mut buf, cx.theme);
    buf
}

#[test]
fn a_pane_draws_and_returns_its_content_area() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 12, 4);
    let mut buf = Buffer::empty(area);
    let inner = Pane::new().title("t").draw(&Context::new(&theme), area, &mut buf);
    assert_eq!(inner, Pane::new().inner(&theme, area));
    assert_ne!(buf[(0, 0)].symbol(), " ", "the border is drawn");
}

#[test]
fn each_motion_keeps_its_own_memory() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 10, 8);
    let (a, b) = (Motion::new(), Motion::new());
    let (cx_a, cx_b) = (Context::with_motion(&theme, &a), Context::with_motion(&theme, &b));
    let rows: Vec<(u16, &str)> = vec![(0, "a"), (1, "b"), (2, "c"), (3, "d"), (4, "e")];
    frame(&cx_a, area, &rows);
    frame(&cx_b, area, &rows);
    // Only `a` sees the list scroll by three rows; `b` draws it where it was.
    let scrolled: Vec<(u16, &str)> = rows.iter().map(|&(y, t)| (y + 3, t)).collect();
    frame(&cx_a, area, &scrolled);
    frame(&cx_b, area, &rows);
    assert!(a.take_redraw(), "the scrolled list glides");
    assert!(!b.take_redraw(), "the still list does not move");
    assert_eq!(b.poll_timeout(Duration::from_secs(1)), Duration::from_secs(1));
}
