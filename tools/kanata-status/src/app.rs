use crate::kbd::{self, Action, Config};
use crate::link::{Event, State};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event as Term, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ui::{Pane, Theme, mix};

const HISTORY_LIMIT: usize = 500;
/// The widest row of the keyboard, in quarter keys (a 1u key is 4).
const ROW_QUARTERS: usize = 60;
const KEY_ROWS_HEIGHT: u16 = 14;

enum Item {
    Press(String),
    Layer(String),
}

pub struct App {
    cfg: Config,
    theme: Theme,
    addr: String,
    state: State,
    connected: bool,
    /// The last layer that is not held on top of another.
    base: String,
    /// A layer picked by hand to look at; `None` follows the live one.
    browsing: Option<usize>,
    history: Vec<Item>,
}

impl App {
    pub fn new(cfg: Config, addr: String) -> App {
        let base = cfg.layers[0].name.clone();
        App {
            cfg,
            theme: Theme::load(),
            addr,
            state: State::default(),
            connected: false,
            base,
            browsing: None,
            history: Vec::new(),
        }
    }

    /// Takes a state from the daemon; true when something visible changed.
    pub fn apply(&mut self, event: Event) -> bool {
        let state = match event {
            Event::Down => {
                let changed = self.connected;
                self.connected = false;
                self.state = State::default();
                return changed;
            }
            Event::State(s) => s,
        };
        self.connected = true;
        if state == self.state {
            return false;
        }
        if !state.layer.is_empty() && state.layer != self.state.layer {
            self.history.push(Item::Layer(state.layer.clone()));
            if self.cfg.layer(&state.layer).is_some() && !self.cfg.is_overlay(&state.layer) {
                self.base = state.layer.clone();
            }
        }
        for key in state.keys.iter().filter(|k| !self.state.keys.contains(k)) {
            self.history.push(Item::Press(key.clone()));
        }
        let excess = self.history.len().saturating_sub(HISTORY_LIMIT);
        self.history.drain(..excess);
        self.state = state;
        true
    }

    /// Handles a terminal event; true to quit. Few keys are bound, and all are Ctrl chords
    /// that main-layer kanata cannot type by accident, since every key pressed here also
    /// reaches this tool.
    pub fn handle_event(&mut self, event: &Term) -> (bool, bool) {
        let Term::Key(key) = event else { return (false, matches!(event, Term::Resize(..))) };
        if key.kind != KeyEventKind::Press || !key.modifiers.contains(KeyModifiers::CONTROL) {
            return (false, false);
        }
        let n = self.cfg.layers.len();
        let shown = self.shown_index();
        match key.code {
            KeyCode::Char('q') => return (true, false),
            KeyCode::Char('n') => self.browsing = Some((shown + 1) % n),
            KeyCode::Char('p') => self.browsing = Some((shown + n - 1) % n),
            KeyCode::Char('l') => self.browsing = None,
            KeyCode::Char('k') => self.history.clear(),
            _ => return (false, false),
        }
        (false, true)
    }

    fn live_index(&self) -> Option<usize> {
        self.cfg.layers.iter().position(|l| l.name == self.state.layer)
    }

    fn shown_index(&self) -> usize {
        self.browsing.or(self.live_index()).unwrap_or_else(|| {
            self.cfg.layers.iter().position(|l| l.name == self.base).unwrap_or(0)
        })
    }

    /// The keys held, matched to the physical keys that produce them. Kanata reports what
    /// it emits, so a key is lit when everything it emits is held and nothing held explains
    /// more of it than another candidate does.
    fn lit(&self, layer: &str) -> Vec<String> {
        let held = &self.state.keys;
        let mut found: Vec<(String, Vec<String>)> = Vec::new();
        for key in self.cfg.rows.iter().flatten() {
            if let Action::Keys { outputs, .. } = self.cfg.action(layer, &self.base, key)
                && !outputs.is_empty()
                && outputs.iter().all(|o| held.contains(o))
            {
                found.push((key.clone(), outputs));
            }
        }
        let subset =
            |a: &[String], b: &[String]| a.len() < b.len() && a.iter().all(|x| b.contains(x));
        found
            .iter()
            .filter(|(_, a)| !found.iter().any(|(_, b)| subset(a, b)))
            .map(|(k, _)| k.clone())
            .collect()
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let l = &self.theme.layout;
        let keyboard_h = KEY_ROWS_HEIGHT + 2 + 2 + 2 * l.padding_y;
        let rows = ui::layout::rows(
            &self.theme,
            area,
            &[Constraint::Length(keyboard_h), Constraint::Min(3), Constraint::Length(1)],
        );
        let rows: Vec<Rect> = rows.iter().map(|r| r.intersection(area)).collect();
        self.render_keyboard(rows[0], buf);
        self.render_history(rows[1], buf);
        if rows[2].is_empty() {
            return;
        }
        let hint = "^q quit   ^n ^p layer   ^l follow live   ^k clear history";
        buf.set_stringn(
            rows[2].x,
            rows[2].y,
            hint,
            rows[2].width as usize,
            Style::new().fg(self.theme.colors.muted.into()),
        );
    }

    fn render_keyboard(&self, area: Rect, buf: &mut Buffer) {
        let c = &self.theme.colors;
        let idx = self.shown_index();
        let layer = self.cfg.layers[idx].name.clone();
        let live = self.browsing.is_none();
        let title = format!("Keyboard  {layer}{}", if live { "" } else { "  (browsing)" });
        let pane = Pane::new(&self.theme).title(&title).focused(true);
        let inner = pane.inner(area);
        pane.render(area, buf);

        let lit = if live { self.lit(&layer) } else { Vec::new() };
        let width = inner.width.min(144) as usize;
        let at = |q: usize| (q * width / ROW_QUARTERS) as u16;
        let base_bg = mix(c.muted.into(), c.bg.into(), 16);
        let lit_bg = mix(c.accent.into(), c.bg.into(), 55);
        for (r, row) in self.cfg.rows.iter().enumerate() {
            let y = inner.y + (r as u16) * 3;
            if y + 1 >= inner.y + inner.height {
                break;
            }
            let fixed: usize = row.iter().filter(|k| *k != "spc").map(|k| quarters(k)).sum();
            let mut cum = 0;
            for key in row {
                let q = if key == "spc" {
                    ROW_QUARTERS.saturating_sub(fixed).max(4)
                } else {
                    quarters(key)
                };
                let (x0, x1) = (at(cum), at(cum + q));
                cum += q;
                let w = (x1 - x0).saturating_sub(1);
                if w == 0 || x0 + w > inner.width {
                    continue;
                }
                let cell = Rect::new(inner.x + x0, y, w, 2);
                let on = lit.contains(key);
                buf.set_style(cell, Style::new().bg(if on { lit_bg } else { base_bg }));
                let action = self.cfg.action(&layer, &self.base, key);
                let (label, color) = self.label(key, &action);
                let src_style = Style::new().fg(if on { c.accent.into() } else { c.muted.into() });
                let mut label_style = Style::new().fg(color);
                if on {
                    label_style = label_style.add_modifier(Modifier::BOLD);
                }
                let max = w as usize;
                buf.set_stringn(cell.x, y, format!(" {}", kbd::short(key)), max, src_style);
                buf.set_stringn(cell.x, y + 1, format!(" {label}"), max, label_style);
            }
        }

        let status_y = inner.y + KEY_ROWS_HEIGHT + 1;
        if status_y < inner.y + inner.height {
            let line = self.status_line();
            Paragraph::new(line).render(Rect::new(inner.x, status_y, inner.width, 1), buf);
        }
    }

    /// The text and color of what a key does.
    fn label(&self, key: &str, action: &Action) -> (String, ratatui::style::Color) {
        let c = &self.theme.colors;
        match action {
            Action::Blocked => ("·".into(), c.muted.into()),
            Action::Transparent => (kbd::short(key), c.muted.into()),
            Action::Keys { label, outputs } if outputs.len() == 1 && outputs[0] == key => {
                (label.clone(), c.muted.into())
            }
            Action::Keys { label, .. } => (label.clone(), c.fg.into()),
            Action::Switch(l) => (format!("→{l}"), c.accent.into()),
            Action::Hold(l) => (format!("{l}↓"), c.accent.into()),
            Action::Other(o) => (o.clone(), c.info.into()),
        }
    }

    fn status_line(&self) -> Line<'static> {
        let c = &self.theme.colors;
        if !self.connected {
            return Line::styled(
                format!("kb daemon not reachable at {}", self.addr),
                Style::new().fg(c.error.into()),
            );
        }
        let mut spans = Vec::new();
        if !self.state.kanata {
            spans.push(Span::styled("kanata not reachable   ", Style::new().fg(c.warning.into())));
        }
        spans.push(Span::styled("held ", Style::new().fg(c.muted.into())));
        if self.state.keys.is_empty() {
            spans.push(Span::styled("none", Style::new().fg(c.muted.into())));
        } else {
            let held =
                self.state.keys.iter().map(|k| kbd::short(k)).collect::<Vec<_>>().join(" + ");
            spans.push(Span::styled(
                held,
                Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD),
            ));
        }
        Line::from(spans)
    }

    fn render_history(&self, area: Rect, buf: &mut Buffer) {
        let c = &self.theme.colors;
        let pane = Pane::new(&self.theme).title("History");
        let inner = pane.inner(area);
        pane.render(area, buf);
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let (width, height) = (inner.width as usize, inner.height as usize);
        let last = self.history.len().saturating_sub(1);
        let mut lines: Vec<Vec<Span>> = vec![Vec::new()];
        let mut used = 0;
        for (i, item) in self.history.iter().enumerate() {
            let (text, style) = match item {
                Item::Press(k) if i == last => {
                    (kbd::short(k), Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD))
                }
                Item::Press(k) => (kbd::short(k), Style::new().fg(c.fg.into())),
                Item::Layer(l) => (format!("[{l}]"), Style::new().fg(c.info.into())),
            };
            let w = text.chars().count() + 2;
            if used + w > width && used > 0 {
                lines.push(Vec::new());
                used = 0;
            }
            used += w;
            let line = lines.last_mut().expect("one line");
            line.push(Span::styled(text, style));
            line.push(Span::raw("  "));
        }
        let skip = lines.len().saturating_sub(height);
        let text: Vec<Line> = lines.into_iter().skip(skip).map(Line::from).collect();
        Paragraph::new(text).render(inner, buf);
    }
}

/// Width of a key in quarter units of a standard ANSI keyboard (a 1u key is 4).
fn quarters(key: &str) -> usize {
    match key {
        "bspc" => 8,
        "tab" | "\\" => 6,
        "caps" => 7,
        "ret" => 9,
        "lsft" => 9,
        "rsft" => 11,
        "lctl" | "lmet" | "lalt" | "ralt" | "rmet" | "rctl" => 5,
        _ => 4,
    }
}

#[cfg(test)]
mod tests;
