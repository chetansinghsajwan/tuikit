use super::*;

fn k(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

const ACTIONS: [Action; 5] = [
    Action { id: "nav.down", name: "down", desc: "move down", basic: false, keys: &["j", "down"] },
    Action {
        id: "view.columns",
        name: "columns",
        desc: "cycle the columns",
        basic: false,
        keys: &["leader v c"],
    },
    Action {
        id: "view.hidden",
        name: "hidden",
        desc: "toggle hidden files",
        basic: false,
        keys: &[".", "leader v h"],
    },
    Action { id: "find", name: "find", desc: "find files", basic: false, keys: &["/", "leader f"] },
    Action {
        id: "page.down",
        name: "page down",
        desc: "half a page",
        basic: false,
        keys: &["ctrl-d"],
    },
];
const GROUPS: [Group; 1] = [Group { prefix: "leader v", name: "view" }];

fn map() -> Keymap {
    Keymap::new("space", &ACTIONS, &GROUPS)
}

#[test]
fn single_keys_and_aliases_run_and_unbound_keys_are_the_callers() {
    let mut m = map();
    assert_eq!(m.feed(&k('j')), Feed::Run("nav.down"));
    assert_eq!(m.feed(&KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)), Feed::Run("nav.down"));
    assert_eq!(m.feed(&k('.')), Feed::Run("view.hidden"));
    assert_eq!(
        m.feed(&KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
        Feed::Run("page.down")
    );
    assert_eq!(m.feed(&k('z')), Feed::Unbound);
    assert!(!m.is_pending());
}

#[test]
fn rebinding_takes_the_key_from_another_action_and_an_empty_list_unbinds() {
    let mut m = map();
    m.rebind("nav.down", &["."]);
    assert_eq!(m.feed(&k('.')), Feed::Run("nav.down"));
    assert_eq!(m.feed(&k('j')), Feed::Unbound);
    m.rebind("find", &[]);
    assert_eq!(m.feed(&k('/')), Feed::Unbound);
    m.rebind("no.such.action", &["z"]);
    assert_eq!(m.feed(&k('z')), Feed::Unbound);
}

#[test]
fn leader_sequences_pend_run_and_cancel() {
    let mut m = map();
    assert_eq!(m.feed(&k(' ')), Feed::Pending);
    assert!(m.is_pending());
    assert_eq!(m.feed(&k('f')), Feed::Run("find"));
    assert!(!m.is_pending());
    // Two deep.
    m.feed(&k(' '));
    assert_eq!(m.feed(&k('v')), Feed::Pending);
    assert_eq!(m.feed(&k('c')), Feed::Run("view.columns"));
    // A wrong key is named and dropped, the menu stays open; Esc cancels it.
    m.feed(&k(' '));
    assert_eq!(m.feed(&k('q')), Feed::NoBinding);
    assert_eq!(m.rejected(), "space q");
    assert!(m.is_pending());
    assert_eq!(m.feed(&KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)), Feed::Cancelled);
    assert!(!m.is_pending());
    // The leader itself is bindable: another leader key works.
    let mut m = Keymap::new(",", &ACTIONS, &GROUPS);
    assert_eq!(m.feed(&k(',')), Feed::Pending);
    assert_eq!(m.feed(&k('f')), Feed::Run("find"));
    assert_eq!(m.feed(&k(' ')), Feed::Unbound);
}

#[test]
fn the_menu_lists_what_can_follow_with_group_names() {
    let mut m = map();
    assert_eq!(m.menu(), None);
    m.feed(&k(' '));
    let menu = m.menu().unwrap();
    assert_eq!(menu.title, "Quick Keys");
    let shown: Vec<(&str, &str, bool)> =
        menu.entries.iter().map(|e| (e.key.as_str(), e.name.as_str(), e.group)).collect();
    assert_eq!(shown, vec![("f", "find", false), ("v", "view", true)]);
    m.feed(&k('v'));
    let menu = m.menu().unwrap();
    assert_eq!(menu.title, "Quick Keys · v");
    let shown: Vec<&str> = menu.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(shown, vec!["columns", "hidden"]);
}

#[test]
fn rebind_replaces_keys_and_ignores_unknown_ids() {
    let mut m = map();
    m.rebind("nav.down", &["n"]);
    assert_eq!(m.feed(&k('n')), Feed::Run("nav.down"));
    assert_eq!(m.feed(&k('j')), Feed::Unbound);
    m.rebind("nope", &["x"]);
    assert_eq!(m.feed(&k('x')), Feed::Unbound);
    assert_eq!(m.keys_of("view.hidden"), ". / space v h");
}

#[test]
fn the_registry_has_no_duplicate_keys_or_ids_and_everything_is_described() {
    for a in &ACTIONS {
        assert!(!a.name.is_empty() && !a.desc.is_empty(), "{}", a.id);
    }
    let mut seen = std::collections::HashSet::new();
    for a in &ACTIONS {
        assert!(seen.insert(a.id), "duplicate id {}", a.id);
    }
    let m = map();
    let mut keys = std::collections::HashSet::new();
    for (seq, id) in &m.bindings {
        assert!(keys.insert(seq.clone()), "key shared by {id}");
    }
}

#[test]
fn rows_and_help_list_every_action_with_its_keys() {
    let m = map();
    let rows = m.rows();
    assert_eq!(rows.len(), ACTIONS.len());
    let hidden = rows.iter().find(|r| r.id == "view.hidden").unwrap();
    assert_eq!(hidden.keys, ". / space v h");
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let mut help = Help::default();
    help.render(area, &mut buf, &Theme::default(), "Keys", &rows);
    let text: String = (0..24)
        .map(|y| (0..80).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n")
        .collect();
    assert!(
        text.contains("Keys") && text.contains("space v c") && text.contains("toggle hidden files"),
        "{text}"
    );
    let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(help.key(&k('j')) && !help.key(&esc));
}

#[test]
fn the_popup_draws_the_title_and_the_entries() {
    let mut m = map();
    m.feed(&k(' '));
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    render_menu(area, &mut buf, &Theme::default(), &m.menu().unwrap());
    let text: String = (0..10)
        .map(|y| (0..40).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n")
        .collect();
    assert!(
        text.contains("Quick Keys")
            && text.contains("find")
            && text.contains("view")
            && text.contains('▸'),
        "{text}"
    );
}

#[test]
fn help_search_filters_rows_and_esc_clears_before_closing() {
    let m = map();
    let rows = m.rows();
    let area = Rect::new(0, 0, 80, 24);
    let text = |h: &mut Help| {
        let mut buf = Buffer::empty(area);
        h.render(area, &mut buf, &Theme::default(), "Keys", &rows);
        (0..24)
            .map(|y| (0..80).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n")
            .collect::<String>()
    };
    let mut help = Help::default();
    assert!(text(&mut help).contains("/ to search"));
    help.key(&k('/'));
    for c in "hidden".chars() {
        help.key(&k(c));
    }
    let t = text(&mut help);
    assert!(
        t.contains("/ hidden") && t.contains("toggle hidden files") && !t.contains("half a page"),
        "{t}"
    );
    // Enter keeps the filter, Esc clears it, a second Esc closes.
    help.key(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(text(&mut help).contains("/ hidden"));
    let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(help.key(&esc));
    assert!(text(&mut help).contains("half a page"));
    assert!(!help.key(&esc));
    // Nothing matches: say so.
    let mut help = Help::default();
    help.key(&k('/'));
    help.key(&k('z'));
    help.key(&k('z'));
    help.key(&k('z'));
    assert!(text(&mut help).contains("no match"));
}
