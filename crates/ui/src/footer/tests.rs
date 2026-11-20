//! Tests for the footer.

use super::*;

#[test]
fn an_error_and_a_plain_message_have_different_colors() {
    let t = Theme::default();
    let ok = Notice::new("staged", false);
    let bad = Notice::new("nothing to commit", true);
    let color = |n: &Notice| Footer::Notice(n).line(&t).spans[0].style.fg;
    assert_ne!(color(&ok), color(&bad));
}

#[test]
fn a_prompt_draws_its_label_then_the_text_and_marks_the_cursor() {
    let t = Theme::default();
    let area = Rect::new(0, 0, 20, 1);
    let mut buf = Buffer::empty(area);
    Footer::Prompt { label: "find: ", text: "ab".into(), cursor: 2 }.render(area, &mut buf, &t);
    let row: String = (0..8).map(|x| buf[(x, 0)].symbol().to_string()).collect();
    assert_eq!(row, "find: ab");
    assert!(buf[(8, 0)].modifier.contains(ratatui::style::Modifier::REVERSED));
}

#[test]
fn a_password_shows_as_dots() {
    assert_eq!(mask("abc"), "•••");
}
