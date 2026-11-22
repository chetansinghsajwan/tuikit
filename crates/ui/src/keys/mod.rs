//! The action registry behind every tool's keys (see `docs/usage-design.md`).
//!
//! A tool declares its actions (id, name, description) and, apart from them, the default keys that run them, and feeds key
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
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
    /// Read one key name such as `j`, `ctrl-d` or `enter`; `None` if it is not a known key.
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

    /// The chord of a real key event, so pressed keys compare with the configured ones.
    pub fn from_event(key: &KeyEvent) -> Chord {
        Chord {
            code: key.code,
            ctrl: key.modifiers.contains(KeyModifiers::CONTROL),
            alt: key.modifiers.contains(KeyModifiers::ALT),
        }
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
        format!(
            "{}{}{base}",
            if self.ctrl { "ctrl-" } else { "" },
            if self.alt { "alt-" } else { "" }
        )
    }
}

/// A named thing a tool can do. Which keys run it is a separate matter (see [`DefaultBinding`]): an
/// action with no binding stays reachable from the command palette.
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
}

/// The default binding of an action: the keys that run it, by its id (see the module docs for the syntax). A binding
/// whose id names no registered action is ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultBinding {
    pub id: &'static str,
    pub keys: &'static [&'static str],
}

/// The actions a keymap offers and the default bindings of some of them. Tools and shells build
/// one from their tables, so actions and bindings travel together without being one thing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Commands {
    pub actions: Vec<Action>,
    pub default_bindings: Vec<DefaultBinding>,
}

impl Commands {
    /// A set from one table of actions and one of bindings.
    pub fn of(actions: &[Action], bindings: &[DefaultBinding]) -> Commands {
        let mut set = Commands::default();
        set.add(actions, bindings);
        set
    }

    /// Adds a table of actions and a table of bindings to the set.
    pub fn add(&mut self, actions: &[Action], bindings: &[DefaultBinding]) {
        self.actions.extend_from_slice(actions);
        self.default_bindings.extend_from_slice(bindings);
    }

    /// What is wrong with the set, one line each (empty when it is sound): an action with no name or
    /// description or an id used twice, a binding for no action or with no keys, a key that does not
    /// parse, or a key bound twice. Every tool's tests assert it is empty.
    pub fn problems(&self) -> Vec<String> {
        let mut found = Vec::new();
        let mut ids = std::collections::HashSet::new();
        for a in &self.actions {
            if a.name.is_empty() || a.desc.is_empty() {
                found.push(format!("{} has no name or description", a.id));
            }
            if !ids.insert(a.id) {
                found.push(format!("duplicate action {}", a.id));
            }
        }
        let mut shown = std::collections::HashSet::new();
        for b in &self.default_bindings {
            if !ids.contains(b.id) {
                found.push(format!("binding for unknown action {}", b.id));
            }
            if b.keys.is_empty() {
                found.push(format!("{} has no keys", b.id));
            }
            for k in b.keys {
                let parsable =
                    k.split_whitespace().all(|w| w == "leader" || Chord::parse(w).is_some());
                if !parsable {
                    found.push(format!("{} has an unparsable key {k}", b.id));
                }
                if !shown.insert(*k) {
                    found.push(format!("key {k} bound twice"));
                }
            }
        }
        found
    }

    /// Adds everything of `other` to the set.
    pub fn extend(&mut self, other: &Commands) {
        self.add(&other.actions, &other.default_bindings);
    }
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

/// The name of the popup that lists what can follow the leader key.
pub const MENU_TITLE: &str = "Quick Keys";

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

/// Resolves key presses to action ids, tracking a multi-key sequence that is under way.
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
    /// A keymap for `commands`, with `leader` (a key name) standing for `leader` in bindings; the
    /// user's configuration (`ui::config`) overrides the leader and the keys.
    pub fn new(leader: &str, commands: &Commands, groups: &[Group]) -> Keymap {
        let leader = crate::config::get()
            .leader
            .as_deref()
            .and_then(Chord::parse)
            .or_else(|| Chord::parse(leader))
            .unwrap_or(Chord { code: KeyCode::Char(' '), ctrl: false, alt: false });
        let mut map = Keymap {
            leader,
            actions: commands.actions.to_vec(),
            groups: Vec::new(),
            bindings: Vec::new(),
            pending: Vec::new(),
            rejected: String::new(),
        };
        for g in groups {
            if let Some(seq) = map.sequence(g.prefix) {
                map.groups.push((seq, g.name));
            }
        }
        for b in &commands.default_bindings {
            if !map.actions.iter().any(|a| a.id == b.id) {
                continue;
            }
            for k in b.keys {
                if let Some(seq) = map.sequence(k) {
                    map.bindings.push((seq, b.id));
                }
            }
        }
        map.apply_config();
        map
    }

    /// The user's leader and bindings (`ui::config`).
    fn apply_config(&mut self) {
        let cfg = crate::config::get();
        for (id, keys) in &cfg.keys {
            let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
            self.rebind(id, &keys);
        }
    }

    /// Read a binding such as `leader v c` into its chords, with `leader` replaced by the
    /// leader key.
    fn sequence(&self, s: &str) -> Option<Vec<Chord>> {
        s.split_whitespace()
            .map(|w| if w == "leader" { Some(self.leader) } else { Chord::parse(w) })
            .collect()
    }

    /// Replace the bindings of `id` (user configuration). Unknown ids and unparsable keys are ignored.
    pub fn rebind(&mut self, id: &str, keys: &[&str]) {
        if !self.actions.iter().any(|a| a.id == id) {
            return;
        }
        self.bindings.retain(|(_, i)| *i != id);
        for k in keys {
            // A key the user gives to this action is no longer another one's.
            if let Some(seq) = self.sequence(k) {
                self.bindings.retain(|(s, i)| !(*s == seq && *i != id));
            }
            if let (Some(seq), Some(a)) =
                (self.sequence(k), self.actions.iter().find(|a| a.id == id))
            {
                self.bindings.push((seq, a.id));
            }
        }
    }

    /// Every action the keymap was built with, for the palette and the help.
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// The keys bound to `id`, as shown (`space v c`, `j / down`).
    pub fn keys_of(&self, id: &str) -> String {
        let shown: Vec<String> = self
            .bindings
            .iter()
            .filter(|(_, i)| *i == id)
            .map(|(seq, _)| seq.iter().map(Chord::label).collect::<Vec<_>>().join(" "))
            .collect();
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

    /// Drop a started sequence, for when the tool closes the menu another way.
    pub fn cancel(&mut self) {
        self.pending.clear();
    }

    /// Take one key press: run a finished binding, continue or cancel a sequence, or return the
    /// key to the caller.
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
        let longer =
            self.bindings.iter().any(|(seq, _)| seq.len() > n && seq[..n] == self.pending[..]);
        if longer {
            return Feed::Pending;
        }
        if n > 1 {
            // A key nothing is bound to inside a menu: name it, drop it, and keep the menu open.
            self.rejected = self.pending.iter().map(Chord::label).collect::<Vec<_>>().join(" ");
            self.pending.pop();
            return Feed::NoBinding;
        }
        self.pending.clear();
        Feed::Unbound
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
                let name = self
                    .groups
                    .iter()
                    .find(|(p, _)| *p == prefix)
                    .map(|(_, name)| *name)
                    .unwrap_or("more");
                entries.push(MenuEntry { key: next.label(), name: name.to_string(), group: true });
            }
        }
        entries.sort_by(|a, b| {
            a.key.to_lowercase().cmp(&b.key.to_lowercase()).then(a.key.cmp(&b.key))
        });
        let deeper: Vec<String> = self.pending.iter().skip(1).map(Chord::label).collect();
        let title = if deeper.is_empty() {
            MENU_TITLE.to_string()
        } else {
            format!("{MENU_TITLE} · {}", deeper.join(" "))
        };
        Some(Menu { title, entries })
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
        self.actions
            .iter()
            .map(|a| Row {
                id: a.id,
                name: a.name.into(),
                keys: self.keys_of(a.id),
                desc: a.desc.into(),
                basic: a.basic,
            })
            .collect()
    }
}

/// The key of `ev` when it is a key press (or repeat), `None` for releases and other events. Terminals
/// that report releases send one per key, which no tool acts on.
pub fn pressed(ev: &Event) -> Option<&KeyEvent> {
    match ev {
        Event::Key(key) if key.kind != KeyEventKind::Release => Some(key),
        _ => None,
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
    let rows: Vec<&Row> = crate::recent::order(
        rows.iter().filter(|r| !r.basic).collect(),
        &crate::recent::load(tool),
        |r| r.id,
    );
    let name_w =
        rows.iter().map(|r| r.name.chars().count()).max().unwrap_or(0).max("Command".len());
    let keys_w =
        rows.iter().map(|r| r.keys.chars().count()).max().unwrap_or(0).min(24).max("Keys".len());
    let mut input = format!("{:<name_w$}  {:<keys_w$}  Description\n", "Command", "Keys");
    for r in &rows {
        input
            .push_str(&format!("{:<name_w$}  {:<keys_w$}  {}\t{}\n", r.name, r.keys, r.desc, r.id));
    }
    let mut child = Command::new("pick")
        .args([
            "--no-preview",
            "--flat",
            "--header-lines",
            "1",
            "--title",
            title,
            "--delimiter",
            "\t",
            "--with-nth",
            "1",
        ])
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
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.query.push(c)
                }
                KeyCode::Down => self.scroll += 1,
                KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
                _ => {}
            }
            return true;
        }
        match key.code {
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => self.scroll += 5,
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll = self.scroll.saturating_sub(5)
            }
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
        rows.iter()
            .filter(|r| {
                q.is_empty()
                    || r.name.to_lowercase().contains(&q)
                    || r.keys.to_lowercase().contains(&q)
                    || r.desc.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Draw the help popup over `area` with `rows` filtered by the search, dimming what is
    /// behind it.
    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        title: &str,
        rows: &[Row],
    ) {
        let rect = crate::layout::popup(theme, area);
        if rect.width < 20 || rect.height < 6 {
            return;
        }
        crate::anim::overlay(buf, area);
        crate::dim::dim(buf, area, theme);
        let c = &theme.colors;
        Clear.render(rect, buf);
        let pane = crate::Pane::new(theme).title(title).focused(true);
        let inner = pane.inner(rect);
        pane.render(rect, buf);
        // The search line and a rule under it, then the rows.
        let hint = if self.searching {
            String::new()
        } else if self.query.is_empty() {
            "/ to search".to_string()
        } else {
            String::new()
        };
        let line = if self.searching || !self.query.is_empty() {
            crate::prompt::line(theme, "/ ", &self.query)
        } else {
            Line::from(Span::styled(hint, Style::new().fg(c.muted.into())))
        };
        Paragraph::new(line).render(Rect { height: 1, ..inner }, buf);
        if self.searching {
            crate::prompt::cursor(
                buf,
                Rect { height: 1, ..inner },
                "/ ",
                self.query.chars().count(),
            );
        }
        buf.set_string(
            rect.x,
            inner.y + 1,
            format!(
                "{}{}{}",
                theme.icons.divider_left.glyph,
                theme.icons.divider.glyph.repeat(usize::from(rect.width).saturating_sub(2)),
                theme.icons.divider_right.glyph,
            ),
            Style::new().fg(c.border_focused.into()),
        );
        let list = Rect { y: inner.y + 2, height: inner.height.saturating_sub(2), ..inner };
        let shown = self.matching(rows);
        if shown.is_empty() {
            buf.set_string(list.x, list.y, "no match", Style::new().fg(c.muted.into()));
            return;
        }
        let keys_w = shown
            .iter()
            .map(|r| r.keys.chars().count())
            .max()
            .unwrap_or(0)
            .min(inner.width as usize / 2);
        let page = usize::from(list.height).max(1);
        self.scroll = self.scroll.min(shown.len().saturating_sub(page));
        for (i, r) in shown.iter().skip(self.scroll).take(page).enumerate() {
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<keys_w$}", r.keys),
                    Style::new().fg(c.warning.into()).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ", Style::new()),
                Span::styled(r.name.to_string(), Style::new().fg(c.accent.into())),
                Span::styled(format!("  {}", r.desc), Style::new().fg(c.muted.into())),
            ]);
            Paragraph::new(line).render(Rect { y: list.y + i as u16, height: 1, ..list }, buf);
        }
    }
}

/// Size of the Quick Keys popup, border included: 44 columns and three quarters of the height
/// (at most 36 rows), the same at every level; smaller only on a small screen.
pub const MENU_WIDTH: u16 = 44;

/// Height of the Quick Keys popup for a screen of this height; see `MENU_WIDTH`.
pub fn menu_height(screen_height: u16) -> u16 {
    (screen_height * 3 / 4).clamp(8, 36).min(screen_height.saturating_sub(1))
}

/// Draw the which-key popup in the centre of `area`: a pane titled with the keys
/// typed so far, one row per option (`key  name`, groups marked with `▸`).
pub fn render_menu(area: Rect, buf: &mut Buffer, theme: &Theme, menu: &Menu) {
    if menu.entries.is_empty() || area.width < 12 || area.height < 4 {
        return;
    }
    crate::anim::overlay(buf, area);
    crate::dim::dim(buf, area, theme);
    let c = &theme.colors;
    let key_w = menu.entries.iter().map(|e| e.key.chars().count()).max().unwrap_or(1);
    // A fixed size, so the popup does not jump between levels (smaller only on a small screen).
    let width = MENU_WIDTH.min(area.width);
    let height = menu_height(area.height);
    let rect = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    Clear.render(rect, buf);
    let pane = crate::Pane::new(theme).title(&menu.title).focused(true);
    let inner = pane.inner(rect);
    pane.render(rect, buf);
    for (i, e) in menu.entries.iter().take(usize::from(inner.height)).enumerate() {
        let mut spans = vec![
            Span::styled(
                format!("{:<key_w$}", e.key),
                Style::new().fg(c.warning.into()).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::new()),
            Span::styled(
                e.name.clone(),
                Style::new().fg(if e.group { c.accent } else { c.fg }.into()),
            ),
        ];
        if e.group {
            spans.push(Span::styled(" ▸", Style::new().fg(c.muted.into())));
        }
        Paragraph::new(Line::from(spans))
            .render(Rect { y: inner.y + i as u16, height: 1, ..inner }, buf);
    }
}

#[cfg(test)]
mod tests;
