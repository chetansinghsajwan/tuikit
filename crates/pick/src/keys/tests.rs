use super::*;

#[test]
fn parses_names_and_matches_events() {
    let k = parse("ctrl-e").unwrap();
    assert_eq!((k.code, k.modifiers), (KeyCode::Char('e'), KeyModifiers::CONTROL));
    assert_eq!(parse("Alt-X").unwrap().modifiers, KeyModifiers::ALT);
    assert_eq!(parse("f2").unwrap().code, KeyCode::F(2));
    assert_eq!(parse("tab").unwrap().code, KeyCode::Tab);
    assert_eq!(parse("ctrl-alt-d").unwrap().modifiers, KeyModifiers::CONTROL | KeyModifiers::ALT);
    assert_eq!(parse("x").unwrap().code, KeyCode::Char('x'));
    for bad in ["", "ctrl-", "f13", "hello", "ctrl-ab"] {
        assert!(parse(bad).is_none(), "{bad}");
    }
    assert!(matches(&KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL), &parse("ctrl-e").unwrap()));
    assert!(!matches(&KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE), &parse("ctrl-e").unwrap()));
}
