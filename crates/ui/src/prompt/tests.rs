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
    let mut s = Suggest::new(vec!["Ada Lovelace".into(), "Grace Hopper".into(), "Alan Adams".into()]);
    s.typed("ad");
    assert_eq!(s.matches(), ["Ada Lovelace", "Alan Adams"]);
    assert_eq!(s.current().as_deref(), Some("Ada Lovelace"));
    assert!(s.step(1));
    assert_eq!(s.current().as_deref(), Some("Alan Adams"));
    assert!(s.step(1));
    assert_eq!(s.current().as_deref(), Some("Ada Lovelace"));
    s.typed("zzz");
    assert!(!s.step(1) && s.current().is_none());
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 6));
    s.typed("");
    s.render(&mut buf, Rect::new(0, 5, 30, 1), &Theme::default());
    assert_eq!(buf[(1, 4)].symbol(), "A");
}

#[test]
fn line_has_the_label_then_the_value() {
    let l = line(&Theme::default(), "new: ", "a.rs");
    assert_eq!(l.spans.iter().map(|s| s.content.as_ref()).collect::<String>(), "new: a.rs");
}
