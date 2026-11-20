//! Tests for `App`: which keys light up, the history log and rendering on tiny screens.

use super::*;

/// A small config with a remapped key, a chord and an insert layer, shared by the tests.
const KBD: &str = "(defsrc\n a s x\n spc)\n(deflayermap main\n a lctl\n s lsft\n x C-x\n ___ XX)\n(deflayermap ins\n ___ _)\n";

/// Builds an `App` from the sample config.
fn app() -> App {
    App::new(Config::parse(KBD).unwrap(), "x".into())
}

/// Builds a daemon state event for a layer with the given keys held.
fn state(layer: &str, keys: &[&str]) -> Event {
    Event::State(State {
        layer: layer.into(),
        keys: keys.iter().map(|k| k.to_string()).collect(),
        kanata: true,
        ..State::default()
    })
}

#[test]
fn the_most_specific_key_is_lit() {
    let mut app = app();
    app.apply(state("main", &["lctl", "x"]));
    assert_eq!(app.lit("main"), vec!["x"]);
    app.apply(state("main", &["lctl"]));
    assert_eq!(app.lit("main"), vec!["a"]);
}

#[test]
fn keys_pass_through_in_the_insert_layer() {
    let mut app = app();
    app.apply(state("ins", &["a", "s"]));
    assert_eq!(app.lit("ins"), vec!["a", "s"]);
}

#[test]
fn history_records_presses_and_layer_changes() {
    let mut app = app();
    app.apply(state("main", &[]));
    app.apply(state("main", &["a"]));
    app.apply(state("main", &[]));
    app.apply(state("ins", &[]));
    let names: Vec<String> = app
        .history
        .iter()
        .map(|i| match i {
            Item::Press(k) => k.clone(),
            Item::Layer(l) => format!("[{l}]"),
        })
        .collect();
    assert_eq!(names, ["[main]", "a", "[ins]"]);
}

#[test]
fn renders_without_panicking_on_small_screens() {
    let mut app = app();
    app.apply(state("main", &["lctl"]));
    for (w, h) in [(120, 40), (60, 20), (10, 5), (1, 1)] {
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        app.render(area, &mut buf);
    }
}

/// Presses a key with the given modifiers through the shell's dispatcher.
fn press(
    d: &mut ui::app::Dispatcher,
    app: &mut App,
    code: ratatui::crossterm::event::KeyCode,
    mods: ratatui::crossterm::event::KeyModifiers,
) -> Request {
    use ratatui::crossterm::event::{Event, KeyEvent};
    d.handle_event(app, &Event::Key(KeyEvent::new(code, mods)), &mut ui::app::NoHost)
}

#[test]
fn only_ctrl_chords_do_anything_and_ctrl_space_opens_the_menu() {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    let mut app = app();
    let mut d = ui::app::Shell::new("kanata-status")
        .leader("ctrl-space")
        .nav(false)
        .actions(crate::ACTIONS)
        .dispatcher(&mut app);
    let ctrl = KeyModifiers::CONTROL;
    // Plain keys reach kanata as well, so they must do nothing here (but `?`, the shell's help).
    for c in ['q', 'n', 'p', 'l', 'k', 'j', ' '] {
        let before = app.browsing;
        assert_eq!(press(&mut d, &mut app, KeyCode::Char(c), KeyModifiers::NONE), Request::None);
        assert_eq!(app.browsing, before, "{c}");
    }
    assert_eq!(press(&mut d, &mut app, KeyCode::Char('?'), KeyModifiers::NONE), Request::Help);
    press(&mut d, &mut app, KeyCode::Char('n'), ctrl);
    assert!(app.browsing.is_some());
    press(&mut d, &mut app, KeyCode::Char('l'), ctrl);
    assert!(app.browsing.is_none());
    assert_eq!(press(&mut d, &mut app, KeyCode::Char(' '), ctrl), Request::None);
    assert!(d.leader_menu(0).is_some(), "ctrl-space starts the leader menu");
    press(&mut d, &mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(press(&mut d, &mut app, KeyCode::Char('q'), ctrl), Request::Quit);
}
