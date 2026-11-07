use super::*;

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn popup() -> Multi {
    Multi::new(
        "Filter",
        vec![
            Section::new("Message", Kind::Text, Vec::new(), Vec::new()),
            Section::new(
                "Author",
                Kind::Many,
                vec!["Ada".into(), "Bob".into(), "Cy".into()],
                Vec::new(),
            ),
            Section::new("Since", Kind::One, vec!["1d".into(), "7d".into()], Vec::new()),
        ],
    )
}

#[test]
fn text_many_and_one_sections_collect_what_is_chosen() {
    let mut m = popup();
    for c in "fix".chars() {
        m.key(&k(KeyCode::Char(c)));
    }
    m.key(&k(KeyCode::Tab));
    m.key(&k(KeyCode::Char(' ')));
    m.key(&k(KeyCode::Down));
    m.key(&k(KeyCode::Down));
    m.key(&k(KeyCode::Char(' ')));
    m.key(&k(KeyCode::Tab));
    m.key(&k(KeyCode::Down));
    m.key(&k(KeyCode::Char(' ')));
    m.key(&k(KeyCode::Up));
    m.key(&k(KeyCode::Char(' ')));
    assert_eq!(m.chosen("Message"), ["fix"]);
    assert_eq!(m.chosen("Author"), ["Ada", "Cy"]);
    assert_eq!(m.chosen("Since"), ["1d"], "one at most: the second choice replaces the first");
    assert_eq!(m.key(&k(KeyCode::Enter)), Outcome::Apply);
}

#[test]
fn typing_narrows_space_toggles_off_and_ctrl_x_clears() {
    let mut m = popup();
    m.key(&k(KeyCode::Tab));
    m.key(&k(KeyCode::Char('b')));
    assert_eq!(m.visible(), ["Bob"]);
    m.key(&k(KeyCode::Char(' ')));
    m.key(&k(KeyCode::Char(' ')));
    assert!(m.chosen("Author").is_empty(), "the second space drops it");
    m.key(&k(KeyCode::Char(' ')));
    m.key(&KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL));
    assert!(m.chosen("Author").is_empty());
    assert_eq!(m.key(&k(KeyCode::Esc)), Outcome::Cancel);
}

#[test]
fn it_draws_the_sections_the_list_and_what_is_chosen() {
    let mut m = popup();
    m.key(&k(KeyCode::Tab));
    m.key(&k(KeyCode::Char(' ')));
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    m.render(area, &mut buf, &Theme::default());
    let text: String = (0..24)
        .map(|y| (0..80).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n")
        .collect();
    assert!(
        text.contains("Author (1)")
            && text.contains("[x] Ada")
            && text.contains("[ ] Bob")
            && text.contains("author: Ada"),
        "{text}"
    );
}
