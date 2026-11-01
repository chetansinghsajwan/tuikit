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
    /// Navigation and other keys everyone knows: left out of the command palette (the `?`
    /// help still lists them).
    pub basic: bool,
    /// Default bindings (see the module docs for the syntax).
    pub keys: &'static [&'static str],
}

/// An action that shows in the command palette.
pub const fn action(id: &'static str, name: &'static str, desc: &'static str, keys: &'static [&'static str]) -> Action {
    Action { id, name, desc, basic: false, keys }
}

/// A basic action (movement and keys everyone knows): left out of the palette, kept in `?`.
pub const fn basic(id: &'static str, name: &'static str, desc: &'static str, keys: &'static [&'static str]) -> Action {
    Action { id, name, desc, basic: true, keys }
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
    /// A started sequence was cancelled with Esc: the key is used up.
    Cancelled,
    /// A started sequence met a key nothing is bound to (`Keymap::rejected` names the sequence): the key is used up.
    NoBinding,
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
    /// The last sequence that ended on a key nothing was bound to.
    rejected: String,
}

impl Keymap {
    /// A keymap for `actions`, with `leader` (a key name) standing for `leader` in bindings.
    pub fn new(leader: &str, actions: &[Action], groups: &[Group]) -> Keymap {
        let leader = Chord::parse(leader).unwrap_or(Chord { code: KeyCode::Char(' '), ctrl: false, alt: false });
        let mut map = Keymap { leader, actions: actions.to_vec(), groups: Vec::new(), bindings: Vec::new(), pending: Vec::new(), rejected: String::new() };
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

    /// The sequence behind the last `Feed::NoBinding` (`space z`), for the warning.
    pub fn rejected(&self) -> &str {
        &self.rejected
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
        if started {
            self.rejected = self.pending.iter().map(Chord::label).collect::<Vec<_>>().join(" ");
        }
        self.pending.clear();
        if started { Feed::NoBinding } else { Feed::Unbound }
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

/// One action as the palette and the help list it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: &'static str,
    pub name: std::borrow::Cow<'static, str>,
    /// The keys bound to it, as shown (`space v c`, `j / down`).
    pub keys: String,
    pub desc: std::borrow::Cow<'static, str>,
    pub basic: bool,
}

impl Keymap {
    /// Every action, in the order the tool declared them.
    pub fn rows(&self) -> Vec<Row> {
        self.actions.iter().map(|a| Row { id: a.id, name: a.name.into(), keys: self.keys_of(a.id), desc: a.desc.into(), basic: a.basic }).collect()
    }
}

/// Let the user pick one of `rows` by name with the shared picker (`pick`, which must be
/// on `PATH`): the command palette of `tool`. Basic rows (movement and the like) are left out, the
/// columns have labels and a rule separates the search line from the results. `None` when
/// cancelled or when `pick` is unavailable. Inside zellij `pick` is a floating pane over the
/// tool, so the caller keeps its screen; elsewhere it gives the terminal to `pick` first.
pub fn choose(tool: &str, rows: &[Row], title: &str) -> Option<&'static str> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    // The commands run last come first (like VS Code and Zed), the rest in the tool's order.
    let rows: Vec<&Row> = crate::recent::order(rows.iter().filter(|r| !r.basic).collect(), &crate::recent::load(tool), |r| r.id);
    let name_w = rows.iter().map(|r| r.name.chars().count()).max().unwrap_or(0).max("Command".len());
    let keys_w = rows.iter().map(|r| r.keys.chars().count()).max().unwrap_or(0).min(24).max("Keys".len());
    let mut input = format!("{:<name_w$}  {:<keys_w$}  Description\n", "Command", "Keys");
    for r in &rows {
        input.push_str(&format!("{:<name_w$}  {:<keys_w$}  {}\t{}\n", r.name, r.keys, r.desc, r.id));
    }
    let mut child = Command::new("pick")
        .args(["--no-preview", "--flat", "--header-lines", "1", "--title", title, "--delimiter", "\t", "--with-nth", "1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(input.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let id = text.lines().next()?.rsplit('\t').next()?;
    let picked = rows.iter().find(|r| r.id == id).map(|r| r.id)?;
    crate::recent::record(tool, picked);
    Some(picked)
}

/// The `?` help: a scrollable popup listing every action with its keys. `/` starts a search
/// that filters the rows by name, keys or description.
#[derive(Debug, Default, Clone)]
pub struct Help {
    scroll: usize,
    query: String,
    searching: bool,
}

impl Help {
    /// Handle a key while the help is open; `false` closes it.
    pub fn key(&mut self, key: &KeyEvent) -> bool {
        if self.searching {
            match key.code {
                KeyCode::Esc => {
                    self.query.clear();
                    self.searching = false;
                }
                KeyCode::Enter => self.searching = false,
                KeyCode::Backspace => {
                    self.query.pop();
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => self.query.push(c),
                KeyCode::Down => self.scroll += 1,
                KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
                _ => {}
            }
            self.scroll = 0.max(self.scroll);
            return true;
        }
        match key.code {
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Down | KeyCode::Char('j') => self.scroll += 1,
            KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::PageDown => self.scroll += 10,
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(10),
            KeyCode::Home | KeyCode::Char('g') => self.scroll = 0,
            // Esc first clears a finished search, then closes.
            KeyCode::Esc if !self.query.is_empty() => self.query.clear(),
            KeyCode::Esc | KeyCode::Char('q' | '?') => return false,
            _ => {}
        }
        true
    }

    /// The rows that match the search (all of them with none).
    fn matching<'a>(&self, rows: &'a [Row]) -> Vec<&'a Row> {
        let q = self.query.to_lowercase();
        rows.iter().filter(|r| q.is_empty() || r.name.to_lowercase().contains(&q) || r.keys.to_lowercase().contains(&q) || r.desc.to_lowercase().contains(&q)).collect()
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme, title: &str, rows: &[Row]) {
        let rect = crate::layout::popup(theme, area);
        if rect.width < 20 || rect.height < 6 {
            return;
        }
        let c = &theme.colors;
        Clear.render(rect, buf);
        let pane = crate::Pane::new(theme).title(title).focused(true);
        let inner = pane.inner(rect);
        pane.render(rect, buf);
        // The search line and a rule under it, then the rows.
        let hint = if self.searching { String::new() } else if self.query.is_empty() { "/ to search".to_string() } else { String::new() };
        let line = if self.searching || !self.query.is_empty() { crate::prompt::line(theme, "/ ", &self.query) } else { Line::from(Span::styled(hint, Style::new().fg(c.muted.into()))) };
        Paragraph::new(line).render(Rect { height: 1, ..inner }, buf);
        if self.searching {
            crate::prompt::cursor(buf, Rect { height: 1, ..inner }, "/ ", self.query.chars().count());
        }
        buf.set_string(rect.x, inner.y + 1, format!("├{}┤", "─".repeat(usize::from(rect.width).saturating_sub(2))), Style::new().fg(c.border_focused.into()));
        let list = Rect { y: inner.y + 2, height: inner.height.saturating_sub(2), ..inner };
        let shown = self.matching(rows);
        if shown.is_empty() {
            buf.set_string(list.x, list.y, "no match", Style::new().fg(c.muted.into()));
            return;
        }
        let keys_w = shown.iter().map(|r| r.keys.chars().count()).max().unwrap_or(0).min(inner.width as usize / 2);
        let page = usize::from(list.height).max(1);
        self.scroll = self.scroll.min(shown.len().saturating_sub(page));
        for (i, r) in shown.iter().skip(self.scroll).take(page).enumerate() {
            let line = Line::from(vec![
                Span::styled(format!("{:<keys_w$}", r.keys), Style::new().fg(c.warning.into()).add_modifier(Modifier::BOLD)),
                Span::styled("  ", Style::new()),
                Span::styled(r.name.to_string(), Style::new().fg(c.accent.into())),
                Span::styled(format!("  {}", r.desc), Style::new().fg(c.muted.into())),
            ]);
            Paragraph::new(line).render(Rect { y: list.y + i as u16, height: 1, ..list }, buf);
        }
    }
}

/// Draw the which-key popup in the centre of `area`: a pane titled with the keys
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
    let rect = Rect { x: area.x + (area.width - width) / 2, y: area.y + (area.height - height) / 2, width, height };
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
        Action { id: "nav.down", name: "down", desc: "move down", basic: false, keys: &["j", "down"] },
        Action { id: "view.columns", name: "columns", desc: "cycle the columns", basic: false, keys: &["leader v c"] },
        Action { id: "view.hidden", name: "hidden", desc: "toggle hidden files", basic: false, keys: &[".", "leader v h"] },
        Action { id: "find", name: "find", desc: "find files", basic: false, keys: &["/", "leader f"] },
        Action { id: "page.down", name: "page down", desc: "half a page", basic: false, keys: &["ctrl-d"] },
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
        assert_eq!(m.feed(&k('q')), Feed::NoBinding);
        assert_eq!(m.rejected(), "space q");
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
        let text: String = (0..24).map(|y| (0..80).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
        assert!(text.contains("Keys") && text.contains("space v c") && text.contains("toggle hidden files"), "{text}");
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
        let text: String = (0..10).map(|y| (0..40).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
        assert!(text.contains("space") && text.contains("find") && text.contains("view") && text.contains('▸'), "{text}");
    }

    #[test]
    fn help_search_filters_rows_and_esc_clears_before_closing() {
        let m = map();
        let rows = m.rows();
        let area = Rect::new(0, 0, 80, 24);
        let text = |h: &mut Help| {
            let mut buf = Buffer::empty(area);
            h.render(area, &mut buf, &Theme::default(), "Keys", &rows);
            (0..24).map(|y| (0..80).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect::<String>()
        };
        let mut help = Help::default();
        assert!(text(&mut help).contains("/ to search"));
        help.key(&k('/'));
        for c in "hidden".chars() {
            help.key(&k(c));
        }
        let t = text(&mut help);
        assert!(t.contains("/ hidden") && t.contains("toggle hidden files") && !t.contains("half a page"), "{t}");
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
}
