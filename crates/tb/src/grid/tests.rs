use super::*;
use ratatui::crossterm::event::KeyModifiers;

fn grid(text: &str) -> Grid {
    Grid::new(Table::parse(text, ','), None, "space")
}

fn press(g: &mut Grid, keys: &str) -> Action {
    let mut last = Action::None;
    for ch in keys.chars() {
        last = g.handle_event(&Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)));
    }
    last
}

fn enter(g: &mut Grid) {
    g.handle_event(&Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));
}

#[test]
fn move_edit_undo_and_redo() {
    let mut g = grid("a,b\n1,2\n");
    press(&mut g, "jl");
    assert_eq!(g.cursor(), (1, 1));
    press(&mut g, "e");
    assert!(g.is_typing());
    press(&mut g, "0");
    enter(&mut g);
    assert_eq!(g.table().value(1, 1), "20");
    assert!(g.modified());
    press(&mut g, "u");
    assert_eq!(g.table().value(1, 1), "2");
    assert!(!g.modified());
    g.handle_event(&Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)));
    assert_eq!(g.table().value(1, 1), "20");
}

#[test]
fn rows_and_columns_are_added_moved_and_deleted_with_confirmation() {
    let mut g = grid("a,b\n1,2\n3,4\n");
    press(&mut g, "j");
    press(&mut g, "d");
    assert_eq!(g.table().row_count(), 3, "asks first");
    press(&mut g, "d");
    assert_eq!(g.table().to_text(), "a,b\n3,4\n");
    press(&mut g, "a");
    assert_eq!(g.cursor(), (2, 0));
    press(&mut g, "L");
    assert_eq!(g.cursor(), (2, 1));
    assert_eq!(g.table().to_text(), "b,a\n4,3\n,\n");
}

#[test]
fn sort_keeps_the_header_and_find_jumps_between_cells() {
    let mut g = grid("n,s\n10,b\n9,a\n");
    press(&mut g, "s");
    assert_eq!(g.table().to_text(), "n,s\n9,a\n10,b\n");
    press(&mut g, "/");
    press(&mut g, "b");
    enter(&mut g);
    assert_eq!(g.cursor(), (2, 1));
}

#[test]
fn quit_asks_when_modified_and_save_writes_the_file() {
    let dir = std::env::temp_dir().join(format!("tb-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.csv");
    std::fs::write(&path, "a,b\n1,2\n").unwrap();
    let mut g = Grid::open(&path, "space").unwrap();
    press(&mut g, "x");
    assert_eq!(press(&mut g, "q"), Action::None, "asks about the unsaved change");
    assert_eq!(press(&mut g, "q"), Action::Quit);
    let mut g = Grid::open(&path, "space").unwrap();
    press(&mut g, "x");
    g.handle_event(&Event::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), ",b\n1,2\n");
    assert!(!g.modified());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fit_cuts_pads_and_right_aligns() {
    assert_eq!(fit("abcdef", 4, false), "abc…");
    assert_eq!(fit("ab", 4, false), "ab  ");
    assert_eq!(fit("12", 4, true), "  12");
}

#[test]
fn it_draws_the_header_the_cursor_and_the_footer() {
    let mut g = grid("name,qty\nfig,3\nplum,12\n");
    let theme = Theme::load();
    let area = Rect::new(0, 0, 40, 8);
    let mut buf = Buffer::empty(area);
    g.render(area, &mut buf, &theme, true);
    let text: String = (0..8)
        .map(|y| (0..40).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n")
        .collect();
    assert!(
        text.contains("name") && text.contains("plum") && text.contains("3 rows × 2 cols"),
        "{text}"
    );
}
