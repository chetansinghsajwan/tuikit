//! The screen: the keyboard drawn for the shown layer, the keys held now, and a history log.

use crate::kbd::{self, Config, KeyAction};
use crate::link::{Event, State};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::sync::mpsc::Receiver;
use ui::app::{Host, Request};
use ui::{Pane, Theme, mix};

/// Cap on the history log, so a long session does not grow memory without bound.
const HISTORY_LIMIT: usize = 500;
/// The widest row of the keyboard, in quarter keys (a 1u key is 4).
const ROW_QUARTERS: usize = 60;

/// The default keys of the screen's actions.
mod binds;
/// The screen's actions.
mod commands;

/// The actions and their default keys together, as a keymap takes them.
pub fn commands() -> ui::keys::Commands {
    ui::keys::Commands::of(commands::ACTIONS, binds::DEFAULT_BINDINGS)
}

/// One line of the history log: a key pressed or a layer entered.
enum Item {
    Press(String),
    Layer(String),
}

/// Everything the screen needs: the parsed config, the latest daemon state and the history.
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
    /// The daemon's events, read on each tick.
    events: Option<Receiver<Event>>,
}

impl App {
    /// Starts on the first layer of the config, not connected, with an empty history.
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
            events: None,
        }
    }

    /// Reads the daemon's events from `events` on each tick of the shell.
    pub fn with_events(mut self, events: Receiver<Event>) -> App {
        self.events = Some(events);
        self
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

    /// Index of the layer kanata reports as active, if the config knows it.
    fn live_index(&self) -> Option<usize> {
        self.cfg.layers.iter().position(|l| l.name == self.state.layer)
    }

    /// Index of the layer to draw: the browsed one, else the live one, else the base layer.
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
            if let KeyAction::Keys { outputs, .. } = self.cfg.action(layer, &self.base, key)
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

    /// Draws the whole screen: layer banner, keyboard, history and the key hint line.
    pub fn render(&self, cx: &ui::Context, area: Rect, buf: &mut Buffer) {
        let l = &self.theme.layout;
        let keyboard_h = self.keys_height() + 2 + 2 + 2 * l.padding_y;
        let rows = ui::layout::rows(
            &self.theme,
            area,
            &[
                Constraint::Length(1),
                Constraint::Length(keyboard_h),
                Constraint::Min(3),
                Constraint::Length(1),
            ],
        );
        let rows: Vec<Rect> = rows.iter().map(|r| r.intersection(area)).collect();
        self.render_layer(rows[0], buf);
        self.render_keyboard(rows[1], buf, cx);
        self.render_history(rows[2], buf, cx);
        if rows[3].is_empty() {
            return;
        }
        let hint = "^q quit   ^n ^p layer   ^l follow live   ^k clear history";
        buf.set_stringn(
            rows[3].x,
            rows[3].y,
            hint,
            rows[3].width as usize,
            Style::new().fg(self.theme.colors.muted.into()),
        );
    }

    /// Height of the key rows: one line per row with a thin line between rows.
    fn keys_height(&self) -> u16 {
        (self.cfg.rows.len() as u16 * 2).saturating_sub(1)
    }

    /// A color for a layer, so each one is recognisable at a glance.
    fn layer_color(&self, name: &str) -> Color {
        let c = &self.theme.colors;
        let palette = [c.success, c.info, c.warning, c.accent, c.error, c.git.renamed];
        let i = self.cfg.layers.iter().position(|l| l.name == name).unwrap_or(0);
        palette[i % palette.len()].into()
    }

    /// Draws the centered layer name banner, marking when a layer is being browsed by hand.
    fn render_layer(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let name = self.cfg.layers[self.shown_index()].name.clone();
        let suffix = if self.browsing.is_some() { " (browsing)" } else { "" };
        let text = format!(" {}{suffix} ", name.to_uppercase());
        let style = Style::new()
            .fg(self.theme.colors.bg.into())
            .bg(self.layer_color(&name))
            .add_modifier(Modifier::BOLD);
        let w = (text.chars().count() as u16).min(area.width);
        buf.set_stringn(area.x + (area.width - w) / 2, area.y, text, w as usize, style);
    }

    /// Draws the keys of the shown layer in a bordered pane, lights the held ones and adds the
    /// status line below.
    fn render_keyboard(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let c = &self.theme.colors;
        let layer = self.cfg.layers[self.shown_index()].name.clone();
        let live = self.browsing.is_none();
        let pane = Pane::new().title("Keyboard").focused(true);
        let inner = pane.draw(cx, area, buf);

        let lit = if live { self.lit(&layer) } else { Vec::new() };
        let width = inner.width.min(144) as usize;
        let at = |q: usize| (q * width / ROW_QUARTERS) as u16;
        let base_bg = mix(c.muted.into(), c.bg.into(), 16);
        let lit_bg = self.layer_color(&layer);
        for (r, row) in self.cfg.rows.iter().enumerate() {
            let y = inner.y + (r as u16) * 2;
            if y >= inner.y + inner.height {
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
                let action = self.cfg.action(&layer, &self.base, key);
                if action == KeyAction::Blocked {
                    continue;
                }
                let w = (x1 - x0).saturating_sub(1);
                if w == 0 || x0 + w > inner.width {
                    continue;
                }
                let cell = Rect::new(inner.x + x0, y, w, 1);
                let (label, color) = self.label(key, &action);
                let style = if lit.contains(key) {
                    Style::new().fg(c.bg.into()).bg(lit_bg).add_modifier(Modifier::BOLD)
                } else {
                    Style::new().fg(color).bg(base_bg)
                };
                buf.set_style(cell, style);
                let label: String = label.to_uppercase().chars().take(w as usize).collect();
                let pad = (w as usize - label.chars().count()) / 2;
                buf.set_stringn(cell.x + pad as u16, y, &label, w as usize - pad, style);
            }
        }

        let status_y = inner.y + self.keys_height() + 1;
        if status_y < inner.y + inner.height {
            let line = self.status_line();
            Paragraph::new(line).render(Rect::new(inner.x, status_y, inner.width, 1), buf);
        }
    }

    /// The text and color of what a key does.
    fn label(&self, key: &str, action: &KeyAction) -> (String, ratatui::style::Color) {
        let c = &self.theme.colors;
        match action {
            KeyAction::Blocked => ("·".into(), c.muted.into()),
            KeyAction::Transparent => (kbd::short(key), c.muted.into()),
            KeyAction::Keys { label, outputs } if outputs.len() == 1 && outputs[0] == key => {
                (kbd::short(label), c.muted.into())
            }
            KeyAction::Keys { label, .. } => (kbd::short(label), c.fg.into()),
            KeyAction::Switch(l) => (format!("→{l}"), c.accent.into()),
            KeyAction::Hold(l) => (format!("{l}↓"), c.accent.into()),
            KeyAction::Other(o) => (o.clone(), c.info.into()),
        }
    }

    /// One line saying whether the daemon or kanata is unreachable and which keys are held.
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
            let mut names: Vec<String> = Vec::new();
            for k in &self.state.keys {
                let name = kbd::short(k).to_uppercase();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            let held = names.join(" + ");
            spans.push(Span::styled(
                held,
                Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD),
            ));
        }
        Line::from(spans)
    }

    /// One line per event, newest at the bottom, like a log.
    fn render_history(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let c = &self.theme.colors;
        let pane = Pane::new().title("History");
        let inner = pane.draw(cx, area, buf);
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let last = self.history.len().saturating_sub(1);
        let skip = self.history.len().saturating_sub(inner.height as usize);
        let lines: Vec<Line> = self
            .history
            .iter()
            .enumerate()
            .skip(skip)
            .map(|(i, item)| match item {
                Item::Press(k) => {
                    let style = if i == last {
                        Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD)
                    } else {
                        Style::new().fg(c.fg.into())
                    };
                    Line::styled(kbd::short(k).to_uppercase(), style)
                }
                Item::Layer(l) => Line::styled(
                    format!("-> {}", l.to_uppercase()),
                    Style::new().fg(self.layer_color(l)),
                ),
            })
            .collect();
        Paragraph::new(lines).render(inner, buf);
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

/// Tests for lighting keys, the history log and rendering.
#[cfg(test)]
mod tests;

/// Lets the shared app shell host the screen: it owns the terminal, the keys, the help and the
/// palette; the screen draws and follows the daemon.
impl ui::app::Screen for App {
    fn render(&mut self, cx: &ui::Context, area: Rect, buf: &mut Buffer) {
        App::render(self, cx, area, buf);
    }

    fn run_action(&mut self, id: &'static str, _host: &mut dyn Host) -> Request {
        let n = self.cfg.layers.len();
        let shown = self.shown_index();
        match id {
            "quit" => return Request::Quit,
            "layer.next" => self.browsing = Some((shown + 1) % n),
            "layer.prev" => self.browsing = Some((shown + n - 1) % n),
            "layer.live" => self.browsing = None,
            "history.clear" => self.history.clear(),
            _ => {}
        }
        Request::None
    }

    /// Applies what the daemon sent since the last tick; true when something visible changed.
    fn on_tick(&mut self) -> bool {
        let events: Vec<Event> =
            self.events.as_ref().map(|rx| rx.try_iter().collect()).unwrap_or_default();
        let mut changed = false;
        for event in events {
            changed |= self.apply(event);
        }
        changed
    }
}
