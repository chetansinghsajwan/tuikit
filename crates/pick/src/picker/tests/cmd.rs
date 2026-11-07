use super::super::*;
use ratatui::crossterm::event::KeyEvent;

fn picker(lines: &[&str]) -> Picker {
    let opts = PickerOptions {
        mode: Mode::Fuzzy,
        paths: false,
        root: std::env::temp_dir(),
        preview: false,
        git: false,
        query: String::new(),
    };
    let mut p = Picker::new(opts, Arc::new(|| {}));
    crate::source::push_candidates(
        &p.injector(),
        lines.iter().map(|l| Candidate::plain(*l)).collect(),
    );
    p.set_cursor(0);
    p
}

fn draw(p: &mut Picker) -> String {
    let area = Rect::new(0, 0, 100, 12);
    let mut b = Buffer::empty(area);
    p.render(area, &mut b, &Theme::default());
    (0..12)
        .map(|y| (0..100).map(|x| b[(x, y)].symbol().to_string()).collect::<String>() + "\n")
        .collect()
}

#[test]
fn plain_lists_have_no_preview_until_a_command_is_set() {
    let mut p = picker(&["alpha one", "beta two"]);
    let before = draw(&mut p);
    assert!(!before.contains("PREVIEW"), "{before}");
    p.set_preview_command("printf 'PREVIEW of %s' {2}", None);
    let after = draw(&mut p);
    assert!(after.contains("PREVIEW of one"), "{after}");
    // Moving the cursor re-runs the command for the new line.
    p.handle_event(&Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    assert!(draw(&mut p).contains("PREVIEW of two"));
}

#[test]
fn command_output_keeps_colors_and_scrolls() {
    let mut p = picker(&["x"]);
    p.set_preview_command(
        "i=1; while [ $i -le 40 ]; do printf '\\033[31mrow %s\\033[0m\\n' $i; i=$((i+1)); done",
        None,
    );
    let first = draw(&mut p);
    assert!(first.contains("row 1") && !first.contains("row 30"), "{first}");
    for _ in 0..6 {
        p.handle_event(&Event::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)));
        draw(&mut p);
    }
    let scrolled = draw(&mut p);
    assert!(
        scrolled.contains("row 40")
            && !scrolled.contains("row 1\n")
            && !scrolled.contains("row 5 "),
        "{scrolled}"
    );
}

#[test]
fn title_overrides_the_default() {
    let mut p = picker(&["a"]);
    assert!(draw(&mut p).contains("pick"));
    p.set_title("History");
    assert!(draw(&mut p).contains("History"));
}

#[test]
fn dirs_only_source_lists_just_folders() {
    let d = std::env::temp_dir().join(format!("pick-dirs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("a/b")).unwrap();
    std::fs::write(d.join("a/file.txt"), "x").unwrap();
    let opts = PickerOptions {
        mode: Mode::Fuzzy,
        paths: true,
        root: d.clone(),
        preview: false,
        git: false,
        query: String::new(),
    };
    let mut p = Picker::new(opts, Arc::new(|| {}));
    crate::source::spawn_dirs(p.injector(), d);
    for _ in 0..200 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        p.nucleo.tick(10);
        if p.nucleo.snapshot().item_count() >= 2 {
            break;
        }
    }
    let mut names: Vec<String> =
        p.nucleo.snapshot().matched_items(..).map(|i| i.data.display.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["a/".to_string(), "a/b/".to_string()]);
}
