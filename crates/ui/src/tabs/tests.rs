use super::*;

#[test]
fn open_close_next_prev_goto_and_reorder() {
    let mut t = Tabs::new("a");
    t.open("b");
    t.open("c"); // a b c, c active
    assert_eq!((t.len(), t.active_index(), *t.active()), (3, 2, "c"));
    t.next();
    assert_eq!(*t.active(), "a"); // wraps
    t.prev();
    assert_eq!(*t.active(), "c");
    assert!(t.goto(1) && *t.active() == "b" && !t.goto(9));
    // A new tab opens next to the active one.
    t.open("x");
    assert_eq!(t.iter().copied().collect::<Vec<_>>(), vec!["a", "b", "x", "c"]);
    assert_eq!(t.active_index(), 2);
    // Reorder: the active tab moves, staying in range.
    t.move_active(-1);
    assert_eq!(t.iter().copied().collect::<Vec<_>>(), vec!["a", "x", "b", "c"]);
    t.move_active(-5);
    assert_eq!((t.active_index(), *t.active()), (0, "x"));
    t.move_active(9);
    assert_eq!((t.active_index(), *t.active()), (3, "x"));
}

#[test]
fn closing_remembers_for_reopen_and_the_last_tab_stays() {
    let mut t = Tabs::new(1);
    assert_eq!(t.close(), None, "the last tab cannot be closed");
    t.open(2);
    t.open(3);
    assert_eq!(t.close(), Some(())); // closes 3, 2 becomes active
    assert_eq!(*t.active(), 2);
    assert!(t.reopen());
    assert_eq!(t.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(*t.active(), 3);
    assert!(!t.reopen());
    t.duplicate();
    assert_eq!(t.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3, 3]);
    for i in 0..20 {
        t.open(100 + i);
        t.close();
    }
    // Only the last few closed tabs are remembered.
    let mut back = 0;
    while t.reopen() {
        back += 1;
    }
    assert_eq!(back, CLOSED_KEPT);
}

fn labels(names: &[&str]) -> Vec<Label> {
    names.iter().map(|n| Label::new(*n)).collect()
}

fn row(buf: &Buffer, width: u16) -> String {
    (0..width).map(|x| buf[(x, 0)].symbol().to_string()).collect::<String>().trim_end().to_string()
}

#[test]
fn all_labels_show_numbered_when_they_fit() {
    let area = Rect::new(0, 0, 60, 1);
    let mut buf = Buffer::empty(area);
    let mut ls = labels(&["README.md", "good.nix"]);
    ls[1].modified = true;
    render(area, &mut buf, &Theme::default(), &ls, 1, true);
    let text = row(&buf, 60);
    assert_eq!(text, " 1 README.md  2 good.nix ●");
    // The active label is bold, the other is not.
    let active = (0..60).find(|&x| buf[(x, 0)].symbol() == "g").unwrap();
    assert!(buf[(active, 0)].modifier.contains(Modifier::BOLD));
    let other = (0..60).find(|&x| buf[(x, 0)].symbol() == "R").unwrap();
    assert!(!buf[(other, 0)].modifier.contains(Modifier::BOLD) && !buf[(active, 0)].modifier.contains(Modifier::UNDERLINED));
    // Without numbers the labels are plain.
    let mut buf = Buffer::empty(area);
    render(area, &mut buf, &Theme::default(), &ls, 0, false);
    assert_eq!(row(&buf, 60), " README.md  good.nix ●");
}

#[test]
fn a_crowded_strip_shows_a_window_around_the_active_tab_with_counts() {
    let names: Vec<String> = (0..9).map(|i| format!("file{i}.rs")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let ls = labels(&refs);
    let area = Rect::new(0, 0, 40, 1);
    let mut buf = Buffer::empty(area);
    render(area, &mut buf, &Theme::default(), &ls, 4, true);
    let text = row(&buf, 40);
    assert!(text.contains("5 file4.rs"), "the active tab is in view: {text:?}");
    assert!(text.starts_with('‹') && text.ends_with('›'), "both ends have counts: {text:?}");
    assert!(text.chars().count() <= 40, "{text:?}");
    // At the first tab nothing is hidden on the left.
    let mut buf = Buffer::empty(area);
    render(area, &mut buf, &Theme::default(), &ls, 0, true);
    let text = row(&buf, 40);
    assert!(text.contains("1 file0.rs") && !text.starts_with('‹') && text.ends_with('›'), "{text:?}");
    // One tab wider than the strip is cut with an ellipsis.
    let mut buf = Buffer::empty(Rect::new(0, 0, 12, 1));
    render(Rect::new(0, 0, 12, 1), &mut buf, &Theme::default(), &labels(&["a-very-long-file-name.rs"]), 0, true);
    let text = row(&buf, 12);
    assert!(text.contains('…') && text.chars().count() <= 12, "{text:?}");
}

#[test]
fn equal_names_get_just_enough_parent_folders() {
    use std::path::Path;
    let t = |ps: &[&str]| short_titles(&ps.iter().map(|p| Path::new(*p)).collect::<Vec<_>>());
    assert_eq!(t(&["/a/src/lib.rs", "/a/ui/lib.rs", "/a/main.rs"]), vec!["src/lib.rs", "ui/lib.rs", "main.rs"]);
    // Deeper clashes need more levels, and only for the paths that clash.
    assert_eq!(t(&["/x/a/b/lib.rs", "/x/c/b/lib.rs", "/x/other.rs"]), vec!["a/b/lib.rs", "c/b/lib.rs", "other.rs"]);
    // Identical paths cannot be told apart: the whole path is shown for both.
    assert_eq!(t(&["/p/q.rs", "/p/q.rs"]), vec!["p/q.rs", "p/q.rs"]);
    assert_eq!(t(&["/"]), vec!["/"]);
    assert_eq!(t(&[]), Vec::<String>::new());
}
