use super::super::*;
use ratatui::crossterm::event::KeyEvent;

fn files(paths: &[&str]) -> Picker {
    let opts = PickerOptions {
        mode: Mode::Fuzzy,
        paths: true,
        root: std::env::temp_dir().join("pick-tree-none"),
        preview: false,
        git: false,
        query: String::new(),
    };
    let mut p = Picker::new(opts, Arc::new(|| {}));
    crate::source::push_candidates(
        &p.injector(),
        paths.iter().map(|s| Candidate::plain(*s)).collect(),
    );
    p.set_cursor(0);
    p.set_tree(true);
    p
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn lines(p: &mut Picker) -> Vec<String> {
    let area = Rect::new(0, 0, 60, 14);
    let mut b = Buffer::empty(area);
    p.render(area, &mut b, &Theme::default());
    (0..14)
        .map(|y| {
            (0..60)
                .map(|x| b[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

fn accept(p: &mut Picker) -> Vec<String> {
    match p.handle_event(&key(KeyCode::Enter)) {
        Action::Accept(v) => v,
        other => panic!("{other:?}"),
    }
}

const SET: [&str; 4] = ["README.md", "src/ui/c.rs", "src/a.rs", "docs/guide/x.md"];

#[test]
fn scrolling_keeps_the_parent_folders_pinned_at_the_top() {
    let names: Vec<String> = (0..40).map(|i| format!("src/ui/f{i:02}.rs")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut p = files(&refs);
    for _ in 0..30 {
        p.handle_event(&key(KeyCode::Down));
    }
    let l = lines(&mut p);
    // Row 0 is the border, 1 the prompt: the list starts at 2 with the merged folder `src/ui`, then files.
    assert!(l[3].contains(" src/ui"), "{l:#?}");
    assert!(l[4].contains(".rs"), "{l:#?}");
    // The cursor row is on screen and nothing is drawn twice.
    assert!(l.iter().any(|r| r.contains("f30.rs")), "{l:#?}");
    assert_eq!(l.iter().filter(|r| r.contains(" src/ui")).count(), 1, "{l:#?}");
}

#[test]
fn list_is_a_hierarchy_with_leaf_names_only() {
    let mut p = files(&SET);
    let l = lines(&mut p);
    let find =
        |s: &str| l.iter().position(|r| r.contains(s)).unwrap_or_else(|| panic!("{s} in {l:#?}"));
    // docs/guide merged into one header; folders first, items after, README last.
    assert!(find(" docs/guide") < find("x.md") && find("x.md") < find(" src"));
    assert!(
        find(" src") < find(" ui")
            && find(" ui") < find("c.rs")
            && find("c.rs") < find("a.rs")
            && find("a.rs") < find("README.md")
    );
    // The folders are headers, so file rows do not repeat them.
    assert!(
        !l.iter().any(|r| r.contains("src/ui/c.rs")
            || r.contains("src/a.rs")
            || r.contains("docs/guide/x.md")),
        "{l:#?}"
    );
    // Indentation: c.rs sits deeper than a.rs.
    let col = |s: &str| {
        let row = &l[find(s)];
        row[..row.find(s).unwrap()].chars().count()
    };
    assert!(col("c.rs") > col("a.rs") && col("a.rs") > col(" src"));
}

#[test]
fn up_and_down_walk_the_items_in_drawing_order_and_accept_prints_the_path() {
    let mut p = files(&SET);
    let _ = lines(&mut p); // drawing builds the hierarchy, which sets where the cursor starts
    // With nothing typed the list starts at its top: the first item as drawn.
    assert_eq!(accept(&mut p), vec!["docs/guide/x.md".to_string()]);
    p.handle_event(&key(KeyCode::Up)); // stays at the first item
    assert_eq!(accept(&mut p), vec!["docs/guide/x.md".to_string()]);
    // Drawing order: docs/guide/x.md, src/ui/c.rs, src/a.rs, README.md.
    p.handle_event(&key(KeyCode::Down));
    assert_eq!(accept(&mut p), vec!["src/ui/c.rs".to_string()]);
    p.handle_event(&key(KeyCode::Down));
    assert_eq!(accept(&mut p), vec!["src/a.rs".to_string()]);
    for _ in 0..9 {
        p.handle_event(&key(KeyCode::Down));
    }
    assert_eq!(accept(&mut p), vec!["README.md".to_string()]); // stops at the last
}

#[test]
fn filtering_keeps_only_matches_and_their_folders_and_starts_on_the_best_match() {
    let mut p = files(&SET);
    for c in "guide".chars() {
        p.handle_event(&key(KeyCode::Char(c)));
    }
    for _ in 0..50 {
        p.nucleo.tick(10);
    }
    let l = lines(&mut p);
    assert!(
        l.iter().any(|r| r.contains("docs/guide")) && l.iter().any(|r| r.contains("x.md")),
        "{l:#?}"
    );
    assert!(!l.iter().any(|r| r.contains("README") || r.contains("c.rs")), "{l:#?}");
    assert_eq!(accept(&mut p), vec!["docs/guide/x.md".to_string()]);

    // With several matches the cursor starts on the best one, wherever it is drawn.
    let mut p = files(&SET);
    for c in "c.rs".chars() {
        p.handle_event(&key(KeyCode::Char(c)));
    }
    for _ in 0..50 {
        p.nucleo.tick(10);
    }
    assert_eq!(accept(&mut p), vec!["src/ui/c.rs".to_string()]);
}

#[test]
fn stays_flat_when_disabled_when_too_big_or_when_candidates_are_not_plain_paths() {
    let mut p = files(&SET);
    p.set_tree(false);
    let l = lines(&mut p);
    assert!(l.iter().any(|r| r.contains("src/ui/c.rs")), "flat list shows full paths: {l:#?}");

    let mut p = files(&SET);
    p.tree_limit = 3; // four results: over the limit
    let l = lines(&mut p);
    assert!(
        l.iter().any(|r| r.contains("src/ui/c.rs")) && !l.iter().any(|r| r.contains("▾")),
        "{l:#?}"
    );

    // A folder candidate (trailing slash) has no place in the hierarchy.
    let mut p = files(&["a/", "a/b.rs"]);
    let l = lines(&mut p);
    assert!(!l.iter().any(|r| r.contains("▾")), "{l:#?}");
}

#[test]
fn grep_results_are_grouped_under_their_file() {
    let d = std::env::temp_dir().join(format!("pick-tree-grep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("src")).unwrap();
    std::fs::write(d.join("src/one.rs"), "needle a\nfiller\nneedle b\n").unwrap();
    std::fs::write(d.join("top.txt"), "needle c\n").unwrap();
    let opts = PickerOptions {
        mode: Mode::Grep,
        paths: true,
        root: d,
        preview: false,
        git: false,
        query: "needle".into(),
    };
    let mut p = Picker::new(opts, Arc::new(|| {}));
    p.set_tree(true);
    for _ in 0..300 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        p.nucleo.tick(10);
        if p.nucleo.snapshot().item_count() >= 3 {
            break;
        }
    }
    let l = lines(&mut p);
    let pos =
        |s: &str| l.iter().position(|r| r.contains(s)).unwrap_or_else(|| panic!("{s} in {l:#?}"));
    // `src/one.rs` is a header (merged chain), its two matches are rows beneath it.
    assert!(
        pos(" src/one.rs") < pos("1: needle a")
            && pos("1: needle a") < pos("3: needle b")
            && pos("3: needle b") < pos(" top.txt"),
        "{l:#?}"
    );
    // Accepting a match still prints path:line:col:text.
    p.handle_event(&key(KeyCode::Down));
    let got = accept(&mut p);
    assert!(got[0].starts_with("src/one.rs:") || got[0].starts_with("top.txt:"), "{got:?}");
}
