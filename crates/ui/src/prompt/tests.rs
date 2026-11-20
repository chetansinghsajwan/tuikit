//! Tests for the prompt line and its suggestions.

use super::*;

#[test]
fn the_cursor_cell_is_reversed_after_the_label() {
    let area = Rect::new(0, 0, 10, 1);
    let mut buf = Buffer::empty(area);
    cursor(&mut buf, area, "> ", 3);
    assert!(buf[(5, 0)].modifier.contains(Modifier::REVERSED));
    assert!(!buf[(4, 0)].modifier.contains(Modifier::REVERSED));
    cursor(&mut buf, area, "> ", 20); // outside: no panic, no mark
}

#[test]
fn suggestions_match_anywhere_prefixes_first_and_step_around() {
    let mut s =
        Suggest::new(vec!["Ada Lovelace".into(), "Grace Hopper".into(), "Alan Adams".into()]);
    s.set_typed_text("ad");
    assert_eq!(s.matches(), ["Ada Lovelace", "Alan Adams"]);
    assert_eq!(s.selected().as_deref(), Some("Ada Lovelace"));
    assert!(s.move_selection(1));
    assert_eq!(s.selected().as_deref(), Some("Alan Adams"));
    assert!(s.move_selection(1));
    assert_eq!(s.selected().as_deref(), Some("Ada Lovelace"));
    s.set_typed_text("zzz");
    assert!(!s.move_selection(1) && s.selected().is_none());
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 6));
    s.set_typed_text("");
    s.render(&mut buf, Rect::new(0, 5, 30, 1), &Theme::default());
    assert_eq!(buf[(1, 4)].symbol(), "A");
}

#[test]
fn line_has_the_label_then_the_value() {
    let l = line(&Theme::default(), "new: ", "a.rs");
    assert_eq!(l.spans.iter().map(|s| s.content.as_ref()).collect::<String>(), "new: a.rs");
}

#[test]
fn a_prompt_edits_submits_and_cancels() {
    use ratatui::crossterm::event::{KeyEvent, KeyModifiers};
    let key = |c| Event::Key(KeyEvent::new(c, KeyModifiers::NONE));
    let mut input = Input::default();
    assert_eq!(typed(&mut input, &key(KeyCode::Char('a'))), Typed::Editing);
    assert_eq!(typed(&mut input, &key(KeyCode::Enter)), Typed::Submit("a".into()));
    assert_eq!(typed(&mut input, &key(KeyCode::Esc)), Typed::Cancel);
}

#[test]
fn only_y_d_and_enter_confirm() {
    assert!(confirms(KeyCode::Char('y')));
    assert!(confirms(KeyCode::Char('D')));
    assert!(confirms(KeyCode::Enter));
    assert!(!confirms(KeyCode::Char('n')));
}
