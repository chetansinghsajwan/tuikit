use super::super::*;
use ratatui::crossterm::event::KeyEvent;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pick-grep-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("a.txt"), "one\nneedle here\nthree\n").unwrap();
    std::fs::write(d.join("b.rs"), "fn needle() {}\n").unwrap();
    std::fs::write(d.join("c.txt"), "nothing\n").unwrap();
    d
}

fn grep_picker(root: PathBuf, query: &str) -> Picker {
    let opts = PickerOptions {
        mode: Mode::Grep,
        paths: false,
        root,
        preview: true,
        git: false,
        query: query.into(),
    };
    Picker::new(opts, Arc::new(|| {}))
}

/// Tick until the item count is `want` and stays there, or give up.
fn wait_items(p: &mut Picker, want: u32) {
    for _ in 0..300 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        p.nucleo.tick(10);
        if p.nucleo.snapshot().item_count() == want {
            std::thread::sleep(std::time::Duration::from_millis(30));
            p.nucleo.tick(10);
            if p.nucleo.snapshot().item_count() == want {
                return;
            }
        }
    }
}

fn type_str(p: &mut Picker, s: &str) {
    for ch in s.chars() {
        p.handle_event(&Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)));
    }
}

#[test]
fn finds_matches_with_location_and_output_format() {
    let mut p = grep_picker(dir("find"), "needle");
    wait_items(&mut p, 2);
    let all: Vec<Candidate> =
        p.nucleo.snapshot().matched_items(..).map(|i| i.data.clone()).collect();
    assert_eq!(all.len(), 2);
    let a = all.iter().find(|c| c.path() == "a.txt").unwrap();
    assert_eq!(a.output(), "a.txt:2:1:needle here");
    let loc = a.loc.as_ref().unwrap();
    assert_eq!((loc.line, loc.col, &loc.text[loc.hit.clone()]), (2, 1, "needle"));
    let b = all.iter().find(|c| c.path() == "b.rs").unwrap();
    assert_eq!(b.output(), "b.rs:1:4:fn needle() {}");
}

#[test]
fn changing_the_query_restarts_the_search() {
    let mut p = grep_picker(dir("requery"), "needle");
    wait_items(&mut p, 2);
    type_str(&mut p, " h"); // "needle h" only matches a.txt
    wait_items(&mut p, 1);
    assert_eq!(p.current().unwrap().path(), "a.txt");
    assert_eq!(
        p.handle_event(&Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))),
        Action::Accept(vec!["a.txt:2:1:needle here".into()])
    );
}

#[test]
fn invalid_regex_falls_back_to_literal() {
    let d = dir("badre");
    std::fs::write(d.join("p.txt"), "call(x)\n").unwrap();
    let mut p = grep_picker(d, "call(");
    wait_items(&mut p, 1);
    assert_eq!(p.current().unwrap().output(), "p.txt:1:1:call(x)");
}

#[test]
fn preview_focuses_the_matching_line() {
    let d = dir("prev");
    let body: String = (1..=60)
        .map(|n| if n == 40 { "needle line\n".to_string() } else { format!("filler {n}\n") })
        .collect();
    std::fs::write(d.join("big.txt"), body).unwrap();
    let mut p = grep_picker(d, "needle line");
    wait_items(&mut p, 1);
    let area = Rect::new(0, 0, 90, 12);
    let mut buf = Buffer::empty(area);
    p.render(area, &mut buf, &Theme::default());
    let text: String = (0..12)
        .map(|y| (0..90).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n")
        .collect();
    assert!(text.contains("big.txt:40"), "{text}");
    assert!(text.contains("needle line"), "{text}");
    assert!(text.contains("filler 38") || text.contains("filler 39"), "{text}");
    assert!(!text.contains("filler 5\n") && !text.contains("filler 1 "), "{text}");
}

#[test]
fn empty_query_lists_nothing() {
    let mut p = grep_picker(dir("empty"), "");
    std::thread::sleep(std::time::Duration::from_millis(50));
    p.nucleo.tick(10);
    assert_eq!(p.nucleo.snapshot().item_count(), 0);
}
