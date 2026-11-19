//! Tests for the app shell.

use super::*;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The tool's own action in the tests.
const OWN: &[Action] = &[crate::keys::action("thing.do", "do", "do the thing", &["t"])];

/// A screen with a cursor over ten rows, that can pretend to be typing.
#[derive(Default)]
struct Fake {
    cursor: isize,
    typing: bool,
    ran: Vec<&'static str>,
    got: Vec<char>,
    unbound: Vec<String>,
    no_nav: bool,
    ctx: usize,
}

impl Screen for Fake {
    fn render(&mut self, _: Rect, _: &mut Buffer, _: &Theme) {}

    fn captures(&self, _ev: &Event) -> bool {
        self.typing
    }

    fn event(&mut self, ev: &Event, _host: &mut dyn Host) -> Request {
        if let Event::Key(KeyEvent { code: KeyCode::Char(c), .. }) = ev {
            self.got.push(*c);
        }
        Request::None
    }

    fn run(&mut self, id: &'static str, _host: &mut dyn Host) -> Request {
        self.ran.push(id);
        Request::None
    }

    fn unbound(&mut self, keys: &str) {
        self.unbound.push(keys.to_string());
    }

    fn nav(&mut self) -> Option<&mut dyn Navigable> {
        if self.no_nav { None } else { Some(self) }
    }

    fn context(&self) -> usize {
        self.ctx
    }
}

impl Navigable for Fake {
    fn move_by(&mut self, delta: isize) {
        self.cursor = (self.cursor + delta).clamp(0, 9);
    }

    fn page(&self) -> usize {
        4
    }

    fn top(&mut self) {
        self.cursor = 0;
    }

    fn bottom(&mut self) {
        self.cursor = 9;
    }
}

/// Presses `key` on the screen: a character, `space`, `pagedown`, or `ctrl-` and a character.
fn press(d: &mut Dispatcher, s: &mut Fake, key: &str) -> Request {
    let (code, mods) = match key {
        "space" => (KeyCode::Char(' '), KeyModifiers::NONE),
        "pagedown" => (KeyCode::PageDown, KeyModifiers::NONE),
        _ => match key.strip_prefix("ctrl-") {
            Some(c) => (KeyCode::Char(c.chars().next().unwrap()), KeyModifiers::CONTROL),
            None => (KeyCode::Char(key.chars().next().unwrap()), KeyModifiers::NONE),
        },
    };
    d.event(s, &Event::Key(KeyEvent::new(code, mods)), &mut NoHost)
}

fn app() -> App {
    App::new("fake").actions(OWN)
}

#[test]
fn nav_is_on_by_default_and_moves_the_screen() {
    let mut s = Fake::default();
    let mut d = app().dispatcher(&mut s);
    press(&mut d, &mut s, "j");
    press(&mut d, &mut s, "j");
    assert_eq!(s.cursor, 2);
    press(&mut d, &mut s, "pagedown");
    assert_eq!(s.cursor, 6);
    press(&mut d, &mut s, "ctrl-k");
    assert_eq!(s.cursor, 1, "a step is five rows by default");
    press(&mut d, &mut s, "G");
    assert_eq!(s.cursor, 9);
    press(&mut d, &mut s, "g");
    assert_eq!(s.cursor, 0);
    assert!(s.ran.is_empty(), "the shell ran every one of these itself");
}

#[test]
fn a_screen_without_movement_or_with_no_nav_gets_no_nav_keys() {
    let mut s = Fake { no_nav: true, ..Fake::default() };
    let mut d = app().dispatcher(&mut s);
    press(&mut d, &mut s, "j");
    assert_eq!(s.got, vec!['j'], "an unbound key goes to the screen");

    let mut s = Fake::default();
    let mut d = app().no_nav().dispatcher(&mut s);
    press(&mut d, &mut s, "j");
    assert_eq!((s.cursor, s.got.as_slice()), (0, &['j'][..]));
}

#[test]
fn ctrl_d_and_ctrl_u_are_not_movement() {
    let mut s = Fake::default();
    let mut d = app().dispatcher(&mut s);
    press(&mut d, &mut s, "ctrl-d");
    press(&mut d, &mut s, "ctrl-u");
    assert_eq!(s.cursor, 0, "they are left to the preview scroll");
}

#[test]
fn the_tools_own_actions_reach_the_screen_and_the_shell_ones_the_loop() {
    let mut s = Fake::default();
    let mut d = app().dispatcher(&mut s);
    assert_eq!(press(&mut d, &mut s, "t"), Request::None);
    assert_eq!(s.ran, vec!["thing.do"]);
    assert_eq!(press(&mut d, &mut s, "?"), Request::Help);
    assert_eq!(press(&mut d, &mut s, "space"), Request::None, "the leader opens the menu");
    assert!(d.menu(0).is_some());
    assert_eq!(press(&mut d, &mut s, "space"), Request::Palette);
}

#[test]
fn a_screen_that_takes_text_gets_every_key_the_leader_included() {
    let mut s = Fake { typing: true, ..Fake::default() };
    let mut d = app().dispatcher(&mut s);
    for k in ["j", "space", "?"] {
        press(&mut d, &mut s, k);
    }
    assert_eq!(s.got, vec!['j', ' ', '?']);
    assert!(d.menu(0).is_none());
    assert_eq!(s.cursor, 0);
}

#[test]
fn a_sequence_nothing_is_bound_to_is_reported_to_the_screen() {
    let mut s = Fake::default();
    let mut d = app().dispatcher(&mut s);
    press(&mut d, &mut s, "space");
    press(&mut d, &mut s, "z");
    assert_eq!(s.unbound.len(), 1);
}

#[test]
fn the_help_lists_the_shared_and_the_own_actions() {
    let mut s = Fake::default();
    let d = app().dispatcher(&mut s);
    let rows = d.rows(0);
    let text = format!("{rows:?}");
    for want in ["up", "page down", "commands", "keys", "do"] {
        assert!(text.contains(want), "{want} is missing from the help");
    }
}

#[test]
#[should_panic(expected = "registered twice")]
fn an_action_id_registered_twice_fails_at_startup() {
    const CLASH: &[Action] = &[crate::keys::action("nav.up", "up", "again", &["x"])];
    let mut s = Fake::default();
    let _ = App::new("fake").actions(CLASH).dispatcher(&mut s);
}

#[test]
fn each_context_has_its_own_keymap() {
    const ONE: &[Action] = &[crate::keys::action("one.do", "one", "in the first", &["x"])];
    const TWO: &[Action] = &[crate::keys::action("two.do", "two", "in the second", &["x"])];
    let mut s = Fake::default();
    let mut d = App::new("fake").contexts(vec![ONE.to_vec(), TWO.to_vec()]).dispatcher(&mut s);
    press(&mut d, &mut s, "x");
    s.ctx = 1;
    press(&mut d, &mut s, "x");
    assert_eq!(s.ran, vec!["one.do", "two.do"]);
    let ids = |rows: Vec<Row>| rows.iter().map(|r| r.id).collect::<Vec<_>>();
    assert!(ids(d.rows(0)).contains(&"one.do") && !ids(d.rows(0)).contains(&"two.do"));
    assert!(ids(d.rows(1)).contains(&"two.do") && !ids(d.rows(1)).contains(&"one.do"));
}
