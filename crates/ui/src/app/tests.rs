//! Tests for the app shell.

use super::*;
use crate::keys::{Action, DefaultBinding};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The tool's own action in the tests.
const OWN: &[Action] = &[Action { id: "thing.do", name: "do", desc: "do the thing", basic: false }];
/// The default keys of [`OWN`].
const OWN_KEYS: &[DefaultBinding] = &[DefaultBinding { id: "thing.do", keys: &["t"] }];

/// A screen with a cursor over ten rows, that can pretend to be typing.
#[derive(Default)]
struct Fake {
    cursor: isize,
    typing: bool,
    ran: Vec<&'static str>,
    presses: usize,
    tab: usize,
    tabs: bool,
    dynamic: bool,
    opened: usize,
    got: Vec<char>,
    unbound: Vec<String>,
    no_nav: bool,
    ctx: usize,
    scroll: Option<PreviewScroll>,
}

impl Screen for Fake {
    fn render(&mut self, _: &crate::Context, _: Rect, _: &mut Buffer) {}

    fn captures_input(&self, _ev: &Event) -> bool {
        self.typing
    }

    fn on_event(&mut self, ev: &Event, _host: &mut dyn Host) -> Request {
        if let Event::Key(KeyEvent { code: KeyCode::Char(c), .. }) = ev {
            self.got.push(*c);
        }
        Request::None
    }

    fn run_action(&mut self, id: &'static str, _host: &mut dyn Host) -> Request {
        self.ran.push(id);
        Request::None
    }

    fn on_keypress(&mut self) {
        self.presses += 1;
    }

    fn on_unbound_keys(&mut self, keys: &str) {
        self.unbound.push(keys.to_string());
    }

    fn navigable(&mut self) -> Option<&mut dyn Navigable> {
        if self.no_nav { None } else { Some(self) }
    }

    fn context(&self) -> usize {
        self.ctx
    }

    fn tabbed(&mut self) -> Option<&mut dyn Tabbed> {
        if self.tabs { Some(self) } else { None }
    }

    fn preview_scroll(&mut self) -> Option<&mut PreviewScroll> {
        self.scroll.as_mut()
    }
}

impl Tabbed for Fake {
    fn tab_next(&mut self) {
        self.tab += 1;
    }

    fn tab_prev(&mut self) {
        self.tab = self.tab.saturating_sub(1);
    }

    fn tab_select(&mut self, index: usize) {
        self.tab = index;
    }

    fn can_open_close_tabs(&self) -> bool {
        self.dynamic
    }

    fn tab_open(&mut self) {
        self.opened += 1;
    }
}

impl Navigable for Fake {
    fn move_by(&mut self, delta: isize) {
        self.cursor = (self.cursor + delta).clamp(0, 9);
    }

    fn page_row_count(&self) -> usize {
        4
    }

    fn move_to_top(&mut self) {
        self.cursor = 0;
    }

    fn move_to_bottom(&mut self) {
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
    d.handle_event(s, &Event::Key(KeyEvent::new(code, mods)), &mut NoHost)
}

fn app() -> Shell {
    Shell::new("fake").commands(Commands::of(OWN, OWN_KEYS))
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
    let mut d = app().with_navigation(false).dispatcher(&mut s);
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
fn a_screen_with_a_preview_gets_the_shared_scroll_keys_and_the_wheel() {
    use ratatui::crossterm::event::{MouseEvent, MouseEventKind};
    let mut scroll = PreviewScroll::default();
    scroll.view(Rect::new(20, 0, 30, 10), 100);
    let mut s = Fake { scroll: Some(scroll), ..Fake::default() };
    let mut d = app().dispatcher(&mut s);
    press(&mut d, &mut s, "ctrl-d");
    assert_eq!(s.scroll.as_ref().unwrap().offset(), 5);
    press(&mut d, &mut s, "ctrl-u");
    assert_eq!(s.scroll.as_ref().unwrap().offset(), 0);
    let wheel = |column| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column,
            row: 3,
            modifiers: KeyModifiers::NONE,
        })
    };
    d.handle_event(&mut s, &wheel(25), &mut NoHost);
    assert_eq!(s.scroll.as_ref().unwrap().offset(), 3);
    d.handle_event(&mut s, &wheel(2), &mut NoHost);
    assert_eq!(s.scroll.as_ref().unwrap().offset(), 3, "over the list, not the preview");
    assert!(s.ran.is_empty(), "the shared actions never reach the tool");
}

#[test]
fn the_tools_own_actions_reach_the_screen_and_the_shell_ones_the_loop() {
    let mut s = Fake::default();
    let mut d = app().dispatcher(&mut s);
    assert_eq!(press(&mut d, &mut s, "t"), Request::None);
    assert_eq!(s.ran, vec!["thing.do"]);
    assert_eq!(press(&mut d, &mut s, "?"), Request::Help);
    assert_eq!(press(&mut d, &mut s, "space"), Request::None, "the leader opens the menu");
    assert!(d.leader_menu(0).is_some());
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
    assert!(d.leader_menu(0).is_none());
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
    let rows = d.palette_rows(0);
    let text = format!("{rows:?}");
    for want in ["up", "page down", "commands", "keys", "do"] {
        assert!(text.contains(want), "{want} is missing from the help");
    }
}

#[test]
#[should_panic(expected = "registered twice")]
fn an_action_id_registered_twice_fails_at_startup() {
    const CLASH: &[Action] = &[Action { id: "nav.up", name: "up", desc: "again", basic: false }];
    const KEYS: &[DefaultBinding] = &[DefaultBinding { id: "nav.up", keys: &["x"] }];
    let mut s = Fake::default();
    let _ = Shell::new("fake").commands(Commands::of(CLASH, KEYS)).dispatcher(&mut s);
}

#[test]
fn each_context_has_its_own_keymap() {
    const ONE: &[Action] =
        &[Action { id: "one.do", name: "one", desc: "in the first", basic: false }];
    const ONE_KEYS: &[DefaultBinding] = &[DefaultBinding { id: "one.do", keys: &["x"] }];
    const TWO: &[Action] =
        &[Action { id: "two.do", name: "two", desc: "in the second", basic: false }];
    const TWO_KEYS: &[DefaultBinding] = &[DefaultBinding { id: "two.do", keys: &["x"] }];
    let mut s = Fake::default();
    let mut d = Shell::new("fake")
        .contexts(vec![Commands::of(ONE, ONE_KEYS), Commands::of(TWO, TWO_KEYS)])
        .dispatcher(&mut s);
    press(&mut d, &mut s, "x");
    s.ctx = 1;
    press(&mut d, &mut s, "x");
    assert_eq!(s.ran, vec!["one.do", "two.do"]);
    let ids = |rows: Vec<Row>| rows.iter().map(|r| r.id).collect::<Vec<_>>();
    assert!(
        ids(d.palette_rows(0)).contains(&"one.do") && !ids(d.palette_rows(0)).contains(&"two.do")
    );
    assert!(
        ids(d.palette_rows(1)).contains(&"two.do") && !ids(d.palette_rows(1)).contains(&"one.do")
    );
}

#[test]
fn the_screen_hears_of_each_key_the_keymap_decides_on_but_not_captured_ones() {
    let mut s = Fake::default();
    let mut d = app().dispatcher(&mut s);
    press(&mut d, &mut s, "j");
    press(&mut d, &mut s, "?");
    assert_eq!(s.presses, 2);
    s.typing = true;
    press(&mut d, &mut s, "j");
    assert_eq!(s.presses, 2);
}

#[test]
fn tabs_get_the_shared_keys_and_the_number_keys() {
    let mut s = Fake { tabs: true, ..Fake::default() };
    let mut d = app().dispatcher(&mut s);
    press(&mut d, &mut s, "]");
    press(&mut d, &mut s, "]");
    assert_eq!(s.tab, 2);
    press(&mut d, &mut s, "[");
    assert_eq!(s.tab, 1);
    press(&mut d, &mut s, "4");
    assert_eq!(s.tab, 3, "an unbound number key goes to that tab");
    assert!(s.got.is_empty(), "the screen never saw it");
    let ids: Vec<_> = d.palette_rows(0).iter().map(|r| r.id).collect();
    assert!(!ids.contains(&"tab.new"), "fixed tabs cannot be opened: {ids:?}");
}

#[test]
fn dynamic_tabs_can_be_opened_and_a_bound_number_key_stays_the_screens() {
    const ONE: &[Action] =
        &[Action { id: "focus.one", name: "one", desc: "focus one", basic: false }];
    const KEYS: &[DefaultBinding] = &[DefaultBinding { id: "focus.one", keys: &["1"] }];
    let mut s = Fake { tabs: true, dynamic: true, ..Fake::default() };
    let mut d = Shell::new("fake").commands(Commands::of(ONE, KEYS)).dispatcher(&mut s);
    press(&mut d, &mut s, "space");
    press(&mut d, &mut s, "t");
    press(&mut d, &mut s, "n");
    assert_eq!(s.opened, 1);
    press(&mut d, &mut s, "1");
    assert_eq!((s.tab, s.ran.as_slice()), (0, &["focus.one"][..]), "the bound key wins");
}

#[test]
fn a_screen_without_tabs_or_with_no_tabs_gets_no_tab_keys() {
    let mut s = Fake::default();
    let mut d = app().dispatcher(&mut s);
    press(&mut d, &mut s, "]");
    assert_eq!(s.got, vec![']']);

    let mut s = Fake { tabs: true, ..Fake::default() };
    let mut d = app().with_tabs(false).dispatcher(&mut s);
    press(&mut d, &mut s, "]");
    assert_eq!((s.tab, s.got.as_slice()), (0, &[']'][..]));
}
