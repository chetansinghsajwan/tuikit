//! The action registry behind every tool's keys (see `docs/usage-design.md`).
//!
//! A tool declares its actions (id, name, description, default keys) and feeds key
//! events to a [`Keymap`]; the keymap resolves single keys and sequences such as
//! `leader v c`, and describes what can follow a started sequence ([`Menu`], the
//! which-key popup drawn by [`render_menu`]). Names and descriptions are what a
//! command palette and `?` help list.
//!
//! Key names: a character (`j`, `G`, `.`), or `space`, `enter`, `esc`, `tab`,
//! `backtab`, `backspace`, `up`, `down`, `left`, `right`, `home`, `end`, `pageup`,
//! `pagedown`, with `ctrl-` or `alt-` in front. `leader` stands for the configured
//! leader key. A binding is one key or several separated by spaces.

use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Widget};

/// One key press, without the parts that do not matter (shift on a character is
/// already in the character).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    code: KeyCode,
    ctrl: bool,
    alt: bool,
}

impl Chord {
    pub fn parse(s: &str) -> Option<Chord> {
        let (mut ctrl, mut alt) = (false, false);
        let mut rest = s;
        loop {
            if let Some(r) = rest.strip_prefix("ctrl-") {
                ctrl = true;
                rest = r;
            } else if let Some(r) = rest.strip_prefix("alt-") {
                alt = true;
                rest = r;
            } else {
                break;
            }
        }
        let code = match rest {
            "space" => KeyCode::Char(' '),
            "enter" => KeyCode::Enter,
            "esc" => KeyCode::Esc,
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "backspace" => KeyCode::Backspace,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            _ => {
                let mut chars = rest.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => KeyCode::Char(c),
                    _ => return None,
                }
            }
        };
        Some(Chord { code, ctrl, alt })
    }

    pub fn from_event(key: &KeyEvent) -> Chord {
        Chord { code: key.code, ctrl: key.modifiers.contains(KeyModifiers::CONTROL), alt: key.modifiers.contains(KeyModifiers::ALT) }
    }

    /// How the key is shown in menus and help: `j`, `space`, `ctrl-d`, `enter`.
    pub fn label(&self) -> String {
        let base = match self.code {
            KeyCode::Char(' ') => "space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Enter => "enter".into(),
            KeyCode::Esc => "esc".into(),
            KeyCode::Tab => "tab".into(),
            KeyCode::BackTab => "backtab".into(),
            KeyCode::Backspace => "backspace".into(),
            KeyCode::Up => "up".into(),
            KeyCode::Down => "down".into(),
            KeyCode::Left => "left".into(),
            KeyCode::Right => "right".into(),
            KeyCode::Home => "home".into(),
            KeyCode::End => "end".into(),
            KeyCode::PageUp => "pageup".into(),
            KeyCode::PageDown => "pagedown".into(),
            other => format!("{other:?}").to_lowercase(),
        };
        format!("{}{}{base}", if self.ctrl { "ctrl-" } else { "" }, if self.alt { "alt-" } else { "" })
    }
}

/// A named thing a tool can do, with its default keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    /// Stable id, `area.verb`: what the tool matches on and what config refers to.
    pub id: &'static str,
    /// Short name, shown in menus and the palette.
    pub name: &'static str,
    pub desc: &'static str,
    /// Default bindings (see the module docs for the syntax).
    pub keys: &'static [&'static str],
}

/// A name for the menu behind a key prefix: `leader v` is "view".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Group {
    pub prefix: &'static str,
    pub name: &'static str,
}

/// What a key press did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feed {
    /// A binding completed: run this action.
    Run(&'static str),
    /// A sequence is under way (see `Keymap::menu`).
    Pending,
    /// A started sequence ended without an action (Esc, or a key nothing is bound to): the key is used up.
    Cancelled,
    /// Not bound: the key is the caller's.
    Unbound,
}

/// One row of the which-key menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuEntry {
    pub key: String,
    pub name: String,
    /// Opens a deeper menu.
    pub group: bool,
}

/// What can follow the keys typed so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu {
    /// The keys typed so far, as shown (`space v`).
    pub title: String,
    pub entries: Vec<MenuEntry>,
}

#[derive(Debug, Clone)]
pub struct Keymap {
    leader: Chord,
    actions: Vec<Action>,
    groups: Vec<(Vec<Chord>, &'static str)>,
    bindings: Vec<(Vec<Chord>, &'static str)>,
    pending: Vec<Chord>,
}

impl Keymap {
    /// A keymap for `actions`, with `leader` (a key name) standing for `leader` in bindings.
    pub fn new(leader: &str, actions: &[Action], groups: &[Group]) -> Keymap {
        let leader = Chord::parse(leader).unwrap_or(Chord { code: KeyCode::Char(' '), ctrl: false, alt: false });
        let mut map = Keymap { leader, actions: actions.to_vec(), groups: Vec::new(), bindings: Vec::new(), pending: Vec::new() };
        for g in groups {
            if let Some(seq) = map.sequence(g.prefix) {
                map.groups.push((seq, g.name));
            }
        }
        for a in actions {
            for k in a.keys {
                if let Some(seq) = map.sequence(k) {
                    map.bindings.push((seq, a.id));
                }
            }
        }
        map
    }

    fn sequence(&self, s: &str) -> Option<Vec<Chord>> {
        s.split_whitespace().map(|w| if w == "leader" { Some(self.leader) } else { Chord::parse(w) }).collect()
    }

    /// Replace the bindings of `id` (user configuration). Unknown ids and unparsable keys are ignored.
    pub fn rebind(&mut self, id: &str, keys: &[&str]) {
        if !self.actions.iter().any(|a| a.id == id) {
            return;
        }
        self.bindings.retain(|(_, i)| *i != id);
        for k in keys {
            if let (Some(seq), Some(a)) = (self.sequence(k), self.actions.iter().find(|a| a.id == id)) {
                self.bindings.push((seq, a.id));
            }
        }
    }

    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// The keys bound to `id`, as shown (`space v c`, `j / down`).
    pub fn keys_of(&self, id: &str) -> String {
        let shown: Vec<String> = self.bindings.iter().filter(|(_, i)| *i == id).map(|(seq, _)| seq.iter().map(Chord::label).collect::<Vec<_>>().join(" ")).collect();
        shown.join(" / ")
    }

    /// Whether a multi-key sequence has been started.
    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn cancel(&mut self) {
        self.pending.clear();
    }

    pub fn feed(&mut self, key: &KeyEvent) -> Feed {
        let chord = Chord::from_event(key);
        if !self.pending.is_empty() && chord.code == KeyCode::Esc && !chord.ctrl && !chord.alt {
            self.pending.clear();
            return Feed::Cancelled;
        }
        self.pending.push(chord);
        if let Some((_, id)) = self.bindings.iter().find(|(seq, _)| *seq == self.pending) {
            self.pending.clear();
            return Feed::Run(id);
        }
        let n = self.pending.len();
        let longer = self.bindings.iter().any(|(seq, _)| seq.len() > n && seq[..n] == self.pending[..]);
        if longer {
            return Feed::Pending;
        }
        let started = n > 1;
        self.pending.clear();
        if started { Feed::Cancelled } else { Feed::Unbound }
    }

    /// The menu for a started sequence, or `None` when nothing is pending.
    pub fn menu(&self) -> Option<Menu> {
        if self.pending.is_empty() {
            return None;
        }
        let n = self.pending.len();
        let mut entries: Vec<MenuEntry> = Vec::new();
        for (seq, id) in &self.bindings {
            if seq.len() <= n || seq[..n] != self.pending[..] {
                continue;
            }
            let next = seq[n];
            if entries.iter().any(|e| e.key == next.label()) {
                continue;
            }
            if seq.len() == n + 1 {
                let name = self.actions.iter().find(|a| a.id == *id).map(|a| a.name).unwrap_or(id);
                entries.push(MenuEntry { key: next.label(), name: name.to_string(), group: false });
            } else {
                let mut prefix = self.pending.clone();
                prefix.push(next);
                let name = self.groups.iter().find(|(p, _)| *p == prefix).map(|(_, name)| *name).unwrap_or("more");
                entries.push(MenuEntry { key: next.label(), name: name.to_string(), group: true });
            }
        }
        entries.sort_by(|a, b| a.key.to_lowercase().cmp(&b.key.to_lowercase()).then(a.key.cmp(&b.key)));
        Some(Menu { title: self.pending.iter().map(Chord::label).collect::<Vec<_>>().join(" "), entries })
    }
}

/// Draw the which-key popup at the bottom right of `area`: a pane titled with the keys
/// typed so far, one row per option (`key  name`, groups marked with `▸`).
pub fn render_menu(area: Rect, buf: &mut Buffer, theme: &Theme, menu: &Menu) {
    if menu.entries.is_empty() || area.width < 12 || area.height < 4 {
        return;
    }
    let c = &theme.colors;
    let key_w = menu.entries.iter().map(|e| e.key.chars().count()).max().unwrap_or(1);
    let body_w = menu.entries.iter().map(|e| key_w + 2 + e.name.chars().count() + if e.group { 2 } else { 0 }).max().unwrap_or(1).max(menu.title.chars().count());
    // Border and the pane's padding on each side.
    let pad = 2 + 2 * theme.layout.padding_x;
    let width = (body_w as u16 + pad).min(area.width);
    let height = (menu.entries.len() as u16 + 2).min(area.height.saturating_sub(1));
    let rect = Rect { x: area.right() - width, y: area.bottom().saturating_sub(height + 1), width, height };
    Clear.render(rect, buf);
    let pane = crate::Pane::new(theme).title(&menu.title).focused(true);
    let inner = pane.inner(rect);
    pane.render(rect, buf);
    for (i, e) in menu.entries.iter().take(usize::from(inner.height)).enumerate() {
        let mut spans = vec![
            Span::styled(format!("{:<key_w$}", e.key), Style::new().fg(c.warning.into()).add_modifier(Modifier::BOLD)),
            Span::styled("  ", Style::new()),
            Span::styled(e.name.clone(), Style::new().fg(if e.group { c.accent } else { c.fg }.into())),
        ];
        if e.group {
            spans.push(Span::styled(" ▸", Style::new().fg(c.muted.into())));
        }
        Paragraph::new(Line::from(spans)).render(Rect { y: inner.y + i as u16, height: 1, ..inner }, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    const ACTIONS: [Action; 5] = [
        Action { id: "nav.down", name: "down", desc: "move down", keys: &["j", "down"] },
        Action { id: "view.columns", name: "columns", desc: "cycle the columns", keys: &["leader v c"] },
        Action { id: "view.hidden", name: "hidden", desc: "toggle hidden files", keys: &[".", "leader v h"] },
        Action { id: "find", name: "find", desc: "find files", keys: &["/", "leader f"] },
        Action { id: "page.down", name: "page down", desc: "half a page", keys: &["ctrl-d"] },
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
        assert_eq!(m.feed(&KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)), Feed::Run("page.down"));
        assert_eq!(m.feed(&k('z')), Feed::Unbound);
        assert!(!m.is_pending());
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
        // A wrong key ends the sequence and is used up; Esc cancels too.
        m.feed(&k(' '));
        assert_eq!(m.feed(&k('q')), Feed::Cancelled);
        assert!(!m.is_pending());
        m.feed(&k(' '));
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
        assert_eq!(menu.title, "space");
        let shown: Vec<(&str, &str, bool)> = menu.entries.iter().map(|e| (e.key.as_str(), e.name.as_str(), e.group)).collect();
        assert_eq!(shown, vec![("f", "find", false), ("v", "view", true)]);
        m.feed(&k('v'));
        let menu = m.menu().unwrap();
        assert_eq!(menu.title, "space v");
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
    fn the_popup_draws_the_title_and_the_entries() {
        let mut m = map();
        m.feed(&k(' '));
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        render_menu(area, &mut buf, &Theme::default(), &m.menu().unwrap());
        let text: String = (0..10).map(|y| (0..40).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
        assert!(text.contains("space") && text.contains("find") && text.contains("view") && text.contains('▸'), "{text}");
    }
}
