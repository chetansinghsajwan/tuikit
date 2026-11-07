use super::*;
use ratatui::crossterm::event::KeyModifiers;

fn sym(name: &str, kind: Kind, line: u32, end: u32, children: Vec<Symbol>) -> Symbol {
    Symbol { name: name.into(), detail: String::new(), kind, line, col: 4, end_line: end, children }
}

fn panel() -> Panel {
    let mut p = Panel::default();
    p.set_symbols(vec![
        sym(
            "Config",
            Kind::Struct,
            0,
            9,
            vec![sym("name", Kind::Field, 1, 1, vec![]), sym("size", Kind::Field, 2, 2, vec![])],
        ),
        sym("load", Kind::Function, 11, 20, vec![]),
    ]);
    p
}

fn key(c: KeyCode) -> Event {
    Event::Key(KeyEvent::new(c, KeyModifiers::NONE))
}

fn press(p: &mut Panel, s: &str) -> Action {
    let mut last = Action::None;
    for c in s.chars() {
        last = p.handle_event(&key(KeyCode::Char(c)));
    }
    last
}

fn draw(p: &mut Panel, focused: bool) -> String {
    let area = Rect::new(0, 0, 40, 10);
    let mut b = Buffer::empty(area);
    p.render(area, &mut b, &Theme::default(), focused);
    (0..10)
        .map(|y| (0..40).map(|x| b[(x, y)].symbol().to_string()).collect::<String>() + "\n")
        .collect()
}

#[test]
fn the_tree_folds_unfolds_and_enter_jumps() {
    let mut p = panel();
    let text = draw(&mut p, true);
    assert!(text.contains("Config") && text.contains("name") && text.contains("load"), "{text}");
    // h folds the struct, l unfolds it.
    press(&mut p, "h");
    assert!(!draw(&mut p, true).contains("name"));
    press(&mut p, "l");
    assert!(draw(&mut p, true).contains("name"));
    // Enter on a field goes to its line and column.
    press(&mut p, "j");
    assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::Goto { line: 1, col: 4 });
    // h on a member goes to the parent.
    press(&mut p, "h");
    assert_eq!(p.rows[p.cursor].name, "Config");
}

#[test]
fn filtering_keeps_matches_and_their_parents_and_esc_clears() {
    let mut p = panel();
    press(&mut p, "/");
    press(&mut p, "siz");
    let text = draw(&mut p, true);
    assert!(
        text.contains("Config")
            && text.contains("size")
            && !text.contains("load")
            && !text.contains("name"),
        "{text}"
    );
    p.handle_event(&key(KeyCode::Enter));
    assert!(draw(&mut p, true).contains("filter: siz"));
    assert_eq!(p.handle_event(&key(KeyCode::Esc)), Action::None, "the first Esc clears the filter");
    assert!(draw(&mut p, true).contains("load"));
    assert_eq!(p.handle_event(&key(KeyCode::Esc)), Action::Close);
}

#[test]
fn without_the_focus_the_cursor_follows_the_editor_line() {
    let mut p = panel();
    p.follow(Some(2));
    draw(&mut p, false);
    assert_eq!(p.rows[p.cursor].name, "size");
    p.follow(Some(15));
    draw(&mut p, false);
    assert_eq!(p.rows[p.cursor].name, "load");
    p.follow(Some(5));
    draw(&mut p, false);
    assert_eq!(p.rows[p.cursor].name, "Config", "the innermost symbol that contains the line");
    assert!(draw(&mut Panel::default(), true).contains("no symbols"));
}
