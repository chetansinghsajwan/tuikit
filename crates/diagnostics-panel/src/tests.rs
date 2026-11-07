use super::*;

fn item(path: &str, line: u32, sev: Severity, msg: &str) -> Item {
    Item {
        path: PathBuf::from("/ws").join(path),
        line,
        col: 3,
        severity: sev,
        message: msg.into(),
        source: Some("lsp".into()),
    }
}

fn key(c: KeyCode) -> Event {
    Event::Key(KeyEvent::new(c, KeyModifiers::NONE))
}

fn sample() -> Vec<Item> {
    vec![
        item("src/b.rs", 20, Severity::Warning, "unused variable `y`"),
        item("src/a.rs", 5, Severity::Error, "expected `;`"),
        item("src/a.rs", 2, Severity::Hint, "consider using `let`"),
        item("src/b.rs", 4, Severity::Error, "mismatched types\nexpected u32"),
        item("README.md", 1, Severity::Info, "a note"),
    ]
}

fn draw(p: &mut Panel) -> Vec<String> {
    let area = Rect::new(0, 0, 70, 14);
    let mut b = Buffer::empty(area);
    p.render(area, &mut b, &Theme::default(), true);
    (0..14)
        .map(|y| {
            (0..70)
                .map(|x| b[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

#[test]
fn groups_by_file_sorted_by_path_then_line_with_counts_in_the_title() {
    let mut p = Panel::new("/ws");
    p.set_items(sample());
    let l = draw(&mut p);
    let pos =
        |s: &str| l.iter().position(|r| r.contains(s)).unwrap_or_else(|| panic!("{s} in {l:#?}"));
    assert!(l[0].contains("diagnostics: 2 errors, 1 warning"), "{}", l[0]);
    assert!(
        pos("README.md") < pos("a note")
            && pos("a note") < pos("src/a.rs")
            && pos("src/a.rs") < pos("2:3")
            && pos("2:3") < pos("5:3")
    );
    assert!(
        pos("5:3") < pos("src/b.rs") && pos("src/b.rs") < pos("4:3") && pos("4:3") < pos("20:3")
    );
    // Only the first line of a message shows; the source follows it.
    assert!(
        l[pos("4:3")].contains("mismatched types") && !l.iter().any(|r| r.contains("expected u32"))
    );
    assert!(l[pos("5:3")].contains("expected `;`") && l[pos("5:3")].contains("lsp"));
    // File rows carry their count.
    let body = |s: &str| l[pos(s)].trim_end().trim_end_matches('│').trim_end().to_string();
    assert!(body("src/a.rs").ends_with('2') && body("README.md").ends_with('1'), "{l:#?}");
}

#[test]
fn navigation_skips_file_rows_and_enter_opens_the_place() {
    let mut p = Panel::new("/ws");
    p.set_items(sample());
    assert_eq!(
        p.handle_event(&key(KeyCode::Enter)),
        Action::Open { path: "/ws/README.md".into(), line: 1, col: 3 }
    );
    p.handle_event(&key(KeyCode::Down));
    p.handle_event(&key(KeyCode::Char('j')));
    assert_eq!(
        p.handle_event(&key(KeyCode::Enter)),
        Action::Open { path: "/ws/src/a.rs".into(), line: 5, col: 3 }
    );
    for _ in 0..9 {
        p.handle_event(&key(KeyCode::Down));
    }
    assert_eq!(
        p.handle_event(&key(KeyCode::Enter)),
        Action::Open { path: "/ws/src/b.rs".into(), line: 20, col: 3 }
    );
    p.handle_event(&key(KeyCode::Char('g')));
    assert_eq!(p.handle_event(&key(KeyCode::Esc)), Action::Close);
}

#[test]
fn the_filter_cycles_and_the_cursor_stays_on_the_same_problem() {
    let mut p = Panel::new("/ws");
    p.set_items(sample());
    // Stand on the warning in b.rs.
    p.handle_event(&key(KeyCode::Char('G')));
    assert!(matches!(p.handle_event(&key(KeyCode::Enter)), Action::Open { line: 20, .. }));
    p.handle_event(&key(KeyCode::Char('f'))); // errors + warnings: the hint and the note vanish
    let l = draw(&mut p);
    assert!(
        l[0].contains("[errors + warnings]")
            && !l.iter().any(|r| r.contains("a note") || r.contains("let")),
        "{l:#?}"
    );
    assert!(
        matches!(p.handle_event(&key(KeyCode::Enter)), Action::Open { line: 20, .. }),
        "same problem"
    );
    p.handle_event(&key(KeyCode::Char('f'))); // errors only: the warning is gone, the cursor lands on a neighbor
    let l = draw(&mut p);
    assert!(
        l[0].contains("[errors]") && !l.iter().any(|r| r.contains("unused variable")),
        "{l:#?}"
    );
    assert!(matches!(p.handle_event(&key(KeyCode::Enter)), Action::Open { .. }));
    p.handle_event(&key(KeyCode::Char('f'))); // back to all
    assert!(draw(&mut p).iter().any(|r| r.contains("a note")));
}

#[test]
fn an_unchanged_list_keeps_state_and_a_changed_one_keeps_the_cursor_on_its_problem() {
    let mut p = Panel::new("/ws");
    p.set_items(sample());
    p.handle_event(&key(KeyCode::Down));
    p.handle_event(&key(KeyCode::Down));
    let at = |p: &mut Panel| match p.handle_event(&key(KeyCode::Enter)) {
        Action::Open { path, line, .. } => (path, line),
        other => panic!("{other:?}"),
    };
    let before = at(&mut p);
    p.set_items(sample()); // same list, any order
    assert_eq!(at(&mut p), before);
    // A new problem appears above: the cursor stays on the problem it was on.
    let mut more = sample();
    more.push(item("src/a.rs", 1, Severity::Error, "new one"));
    p.set_items(more);
    assert_eq!(at(&mut p), before);
    // The problem it was on is fixed: the cursor stays in range.
    p.set_items(vec![item("x.rs", 1, Severity::Error, "only")]);
    assert_eq!(at(&mut p).1, 1);
    p.set_items(vec![]);
    assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::None);
    assert!(draw(&mut p).iter().any(|r| r.contains("no problems")));
}

#[test]
fn long_messages_are_cut_with_an_ellipsis() {
    let mut p = Panel::new("/ws");
    p.set_items(vec![item("a.rs", 1, Severity::Error, &"very long message ".repeat(20))]);
    let l = draw(&mut p);
    assert!(l.iter().any(|r| r.contains('…') && r.contains("very long")), "{l:#?}");
    assert!(l.iter().all(|r| r.chars().count() <= 70));
}
