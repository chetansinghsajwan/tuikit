use super::super::*;
use crate::candidate::Buffer as BufferInfo;
use ratatui::crossterm::event::KeyEvent;

fn buf(value: &str, flags: &str, path: Option<&str>, focus: Option<u32>, text: &str) -> Candidate {
    Candidate::buffer(
        path.unwrap_or("[scratch]"),
        BufferInfo { value: value.into(), path: path.map(Into::into), flags: flags.into(), focus, text: text.into() },
    )
}

fn picker(root: std::path::PathBuf, items: Vec<Candidate>) -> Picker {
    let opts = PickerOptions { mode: Mode::Fuzzy, paths: true, root, preview: true, git: false, query: String::new() };
    let mut p = Picker::new(opts, Arc::new(|| {}));
    let inj = p.injector();
    for c in items {
        inj.push(c, |c, cols| cols[0] = c.display.as_str().into());
    }
    for _ in 0..100 {
        if !p.nucleo.tick(10).running {
            break;
        }
    }
    p.set_cursor(0);
    p
}

fn draw(p: &mut Picker) -> String {
    let area = Rect::new(0, 0, 100, 14);
    let mut b = Buffer::empty(area);
    p.render(area, &mut b, &Theme::default());
    (0..14).map(|y| (0..100).map(|x| b[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
}

#[test]
fn buffers_show_flags_preview_unsaved_text_and_print_their_id() {
    let d = std::env::temp_dir().join(format!("pick-buf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("a.txt"), "on disk\n").unwrap();
    let mut p = picker(
        d,
        vec![
            buf("7", "+*", Some("a.txt"), Some(2), "first\nUNSAVED edit\nthird\n"),
            buf("3", "", None, None, "scratch text\n"),
        ],
    );
    let text = draw(&mut p);
    assert!(text.contains("+*") && text.contains("a.txt") && text.contains("[scratch]"), "{text}");
    assert!(text.contains("UNSAVED edit") && !text.contains("on disk"), "{text}");
    // Accepting returns the id the host gave, not the display text.
    assert_eq!(
        p.handle_event(&Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))),
        Action::Accept(vec!["7".into()])
    );
}

#[test]
fn set_cursor_preselects_the_previous_buffer_and_clamps() {
    let d = std::env::temp_dir();
    let mut p = picker(
        d.clone(),
        vec![
            buf("1", "*", Some("a"), None, "x"),
            buf("2", "", Some("b"), None, "y"),
            buf("3", "", Some("c"), None, "z"),
        ],
    );
    p.set_cursor(1);
    assert_eq!(
        p.handle_event(&Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))),
        Action::Accept(vec!["2".into()])
    );
    p.set_cursor(99);
    assert_eq!(
        p.handle_event(&Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))),
        Action::Accept(vec!["3".into()])
    );
}
