use super::super::*;
use ratatui::crossterm::event::KeyEvent;

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn grep_toggles_and_file_filters_narrow_the_search() {
    let d = std::env::temp_dir().join(format!("pick-grepf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("src")).unwrap();
    std::fs::create_dir_all(d.join("tests")).unwrap();
    std::fs::write(d.join("src/a.rs"), "Needle here\nneedles too\n").unwrap();
    std::fs::write(d.join("tests/b.rs"), "needle in tests\n").unwrap();
    std::fs::write(d.join("src/c.txt"), "needle in text\n").unwrap();
    let opts = PickerOptions { mode: Mode::Grep, paths: true, root: d.clone(), preview: false, git: false, query: "needle".into() };
    let mut p = Picker::new(opts, Arc::new(|| {}));
    let alt = |c: char| Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT));
    let key = |c: KeyCode| Event::Key(KeyEvent::new(c, KeyModifiers::NONE));
    let count = |p: &mut Picker| {
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            p.tick();
        }
        p.nucleo.snapshot().item_count()
    };
    p.reparse();
    assert_eq!(count(&mut p), 4, "smart case finds all four");
    // Whole word: `needles` goes.
    p.handle_event(&alt('w'));
    assert_eq!(count(&mut p), 3);
    p.handle_event(&alt('w'));
    // Exact case: `Needle` goes (the query is lower case).
    p.handle_event(&alt('c'));
    assert_eq!(count(&mut p), 3);
    p.handle_event(&alt('c'));
    // Include only Rust files, then leave the tests out.
    p.handle_event(&alt('i'));
    for ch in "*.rs".chars() {
        p.handle_event(&key(KeyCode::Char(ch)));
    }
    assert_eq!(count(&mut p), 3, "the text file is out");
    p.handle_event(&key(KeyCode::Enter));
    p.handle_event(&alt('e'));
    for ch in "tests/".chars() {
        p.handle_event(&key(KeyCode::Char(ch)));
    }
    assert_eq!(count(&mut p), 2, "only src/a.rs is left");
    let _ = std::fs::remove_dir_all(&d);
}

fn picker(items: &[&str], paths: bool, root: PathBuf) -> Picker {
    let opts = PickerOptions { mode: Mode::Fuzzy, paths, root, preview: paths, git: false, query: String::new() };
    let p = Picker::new(opts, Arc::new(|| {}));
    let inj = p.injector();
    for i in items {
        inj.push(Candidate::plain(*i), |c, cols| cols[0] = c.display.as_str().into());
    }
    settle(p)
}

fn settle(mut p: Picker) -> Picker {
    for _ in 0..200 {
        if !p.nucleo.tick(20).running {
            break;
        }
    }
    p
}

fn type_str(p: &mut Picker, s: &str) {
    for ch in s.chars() {
        p.handle_event(&key(KeyCode::Char(ch)));
    }
    for _ in 0..200 {
        if !p.nucleo.tick(20).running {
            break;
        }
    }
}

#[test]
fn filters_and_accepts_highlighted() {
    let mut p = picker(&["alpha", "beta", "gamma"], false, PathBuf::new());
    assert_eq!(p.matched(), 3);
    type_str(&mut p, "bet");
    assert_eq!(p.matched(), 1);
    assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::Accept(vec!["beta".into()]));
}

#[test]
fn navigation_clamps_and_tab_marks() {
    let mut p = picker(&["a", "b", "c"], false, PathBuf::new());
    p.handle_event(&key(KeyCode::Up));
    assert_eq!(p.cursor, 0);
    p.handle_event(&key(KeyCode::Tab)); // marks the first and moves down
    p.handle_event(&key(KeyCode::Tab)); // marks the second
    assert_eq!(p.cursor, 2);
    let Action::Accept(v) = p.handle_event(&key(KeyCode::Enter)) else { panic!() };
    assert_eq!(v.len(), 2);
    for _ in 0..10 {
        p.handle_event(&key(KeyCode::Down));
    }
    assert_eq!(p.cursor, 2);
}

#[test]
fn a_live_source_is_asked_again_for_every_query_and_its_results_are_not_refiltered() {
    let mut p = picker(&[], false, PathBuf::new());
    // The source answers with two entries that contain the query, and none for an empty one.
    p.set_live(Box::new(|q, inj, _cancel| {
        if !q.is_empty() {
            for suffix in ["one", "two"] {
                inj.push(Candidate::plain(format!("{q}-{suffix}")), |c, cols| cols[0] = c.display.as_str().into());
            }
        }
    }));
    type_str(&mut p, "ab");
    for _ in 0..50 {
        p.nucleo.tick(10);
        if p.nucleo.snapshot().item_count() == 2 {
            break;
        }
    }
    let names: Vec<String> = p.nucleo.snapshot().matched_items(..).map(|i| i.data.display.clone()).collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.iter().all(|n| n.starts_with("ab-")), "{names:?}");
    // Another character: the old answers are gone, the new ones are in.
    type_str(&mut p, "c");
    for _ in 0..50 {
        p.nucleo.tick(10);
    }
    let names: Vec<String> = p.nucleo.snapshot().matched_items(..).map(|i| i.data.display.clone()).collect();
    assert!(names.iter().all(|n| n.starts_with("abc-")) && names.len() == 2, "{names:?}");
}

#[test]
fn escape_cancels_and_no_match_accepts_nothing() {
    let mut p = picker(&["a"], false, PathBuf::new());
    assert_eq!(p.handle_event(&key(KeyCode::Esc)), Action::Cancel);
    type_str(&mut p, "zzz");
    assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::Accept(vec![]));
}

#[test]
fn renders_list_and_preview() {
    let d = std::env::temp_dir().join(format!("pick-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("hello.txt"), "needle in a haystack\n").unwrap();
    std::fs::write(d.join("other.txt"), "x\n").unwrap();
    let mut p = picker(&["hello.txt", "other.txt"], true, d);
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    p.render(area, &mut buf, &Theme::default());
    let text: String = (0..10).map(|y| (0..80).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
    assert!(text.contains("hello.txt") && text.contains("other.txt"), "{text}");
    assert!(text.contains("needle in a haystack"), "{text}");
    assert!(text.contains("2/2"), "{text}");
}
