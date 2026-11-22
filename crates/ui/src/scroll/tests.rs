//! Tests for the preview scroll.

use super::*;
use ratatui::crossterm::event::KeyModifiers;

fn pane(len: usize) -> PreviewScroll {
    let mut s = PreviewScroll::default();
    s.view(Rect::new(10, 5, 40, 10), len);
    s
}

fn wheel(kind: MouseEventKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE })
}

#[test]
fn half_pages_stop_at_both_ends() {
    let mut s = pane(40);
    s.half_down();
    assert_eq!(s.offset(), 5);
    for _ in 0..20 {
        s.half_down();
    }
    assert_eq!(s.offset(), 30, "the last line sits at the bottom");
    for _ in 0..20 {
        s.half_up();
    }
    assert_eq!(s.offset(), 0);
}

#[test]
fn a_preview_shorter_than_the_pane_does_not_scroll() {
    let mut s = pane(4);
    s.half_down();
    s.bottom();
    assert_eq!(s.offset(), 0);
}

#[test]
fn view_pulls_the_offset_back_when_the_preview_shrinks() {
    let mut s = pane(100);
    s.bottom();
    assert_eq!(s.offset(), 90);
    s.view(Rect::new(10, 5, 40, 10), 25);
    assert_eq!(s.offset(), 15);
}

#[test]
fn window_shows_the_pane_rows_from_the_offset() {
    let mut s = pane(30);
    s.to(12);
    let lines: Vec<usize> = (0..30).collect();
    assert_eq!(s.window(&lines), &lines[12..22]);
}

#[test]
fn actions_run_by_id() {
    let mut s = pane(50);
    assert!(s.run("preview.down"));
    assert_eq!(s.offset(), 5);
    assert!(s.run("preview.bottom"));
    assert_eq!(s.offset(), 40);
    assert!(s.run("preview.top"));
    assert!(!s.run("nav.up"));
    assert!(ACTIONS.iter().all(|a| s.run(a.id)));
}

#[test]
fn the_wheel_scrolls_only_over_the_pane() {
    let mut s = pane(50);
    assert!(s.wheel(&wheel(MouseEventKind::ScrollDown, 20, 8)));
    assert_eq!(s.offset(), 3);
    assert!(s.wheel(&wheel(MouseEventKind::ScrollUp, 20, 8)));
    assert_eq!(s.offset(), 0);
    assert!(!s.wheel(&wheel(MouseEventKind::ScrollDown, 2, 8)), "over the list");
    assert!(!s.wheel(&wheel(MouseEventKind::Moved, 20, 8)));
    assert_eq!(s.offset(), 0);
}
