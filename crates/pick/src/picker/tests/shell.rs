use super::super::*;
use crate::source::LineFormat;
use ratatui::crossterm::event::KeyEvent;

fn picker_from(lines: &[&str], fmt: LineFormat) -> Picker {
    let opts = PickerOptions { mode: Mode::Fuzzy, paths: false, root: std::env::temp_dir(), preview: false, git: false, query: String::new() };
    let mut p = Picker::new(opts, Arc::new(|| {}));
    let items: Vec<Candidate> = lines
        .iter()
        .map(|l| match fmt.with_nth {
            Some(n) => Candidate::with_raw(l.split_whitespace().nth(n - 1).unwrap_or(""), *l),
            None => Candidate::plain(*l),
        })
        .collect();
    crate::source::push_candidates(&p.injector(), items);
    p.set_cursor(0);
    p
}

fn key(code: KeyCode, m: KeyModifiers) -> Event {
    Event::Key(KeyEvent::new(code, m))
}

fn draw(p: &mut Picker) -> String {
    let area = Rect::new(0, 0, 100, 12);
    let mut b = Buffer::empty(area);
    p.render(area, &mut b, &Theme::default());
    (0..12).map(|y| (0..100).map(|x| b[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
}

#[test]
fn expect_keys_accept_and_report_the_key() {
    let mut p = picker_from(&["alpha", "beta"], LineFormat::default());
    p.set_expect(&["ctrl-e".to_string(), "f2".to_string()]).unwrap();
    assert_eq!(p.handle_event(&key(KeyCode::Char('e'), KeyModifiers::CONTROL)), Action::AcceptWith("ctrl-e".into(), vec!["alpha".into()]));
    p.handle_event(&key(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(p.handle_event(&key(KeyCode::F(2), KeyModifiers::NONE)), Action::AcceptWith("f2".into(), vec!["beta".into()]));
    // Enter still accepts normally, and a plain `e` still types.
    assert_eq!(p.handle_event(&key(KeyCode::Enter, KeyModifiers::NONE)), Action::Accept(vec!["beta".into()]));
    assert!(p.set_expect(&["bogus-key".to_string()]).is_err());
}

#[test]
fn header_lines_sit_under_the_prompt_and_are_not_candidates() {
    let mut p = picker_from(&["row one", "row two"], LineFormat::default());
    p.set_header(vec!["ID   NAME   STATUS".to_string()]);
    let text = draw(&mut p);
    let lines: Vec<&str> = text.lines().collect();
    let h = lines.iter().position(|l| l.contains("ID   NAME   STATUS")).expect("header drawn");
    let r = lines.iter().position(|l| l.contains("row one")).expect("rows drawn");
    assert!(h < r && lines[1].contains('>'), "{text}");
    assert!(text.contains("2/2"), "{text}");
}

#[test]
fn with_nth_shows_one_field_but_prints_and_previews_the_whole_line() {
    let mut p = picker_from(&["nginx\t/nix/store/aaa-nginx", "curl\t/nix/store/bbb-curl"], LineFormat { with_nth: Some(1), delimiter: None, timestamps: false });
    p.set_preview_command("printf 'PATH=%s' {2}", None);
    let text = draw(&mut p);
    assert!(text.contains("nginx") && !text.contains("aaa-nginx\n") , "{text}");
    assert!(text.contains("PATH=/nix/store/aaa-nginx"), "{text}");
    assert_eq!(p.handle_event(&key(KeyCode::Enter, KeyModifiers::NONE)), Action::Accept(vec!["nginx\t/nix/store/aaa-nginx".into()]));
}
