use super::chord::Chord;
use super::keymap::Keymap;
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Widget};

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

impl Keymap {
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
