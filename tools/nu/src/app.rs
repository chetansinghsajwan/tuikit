use crate::keys;
use nu_core::{Backend, Network, Security, Status};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;
use ui::keys::{Feed, Keymap};
use ui::{Pane, Theme};

/// How often the list is reread while the screen is open.
const REFRESH: Duration = Duration::from_secs(4);

/// What the host should do after an event.
#[derive(Debug, PartialEq, Eq)]
pub enum Quit {
    No,
    Yes,
    /// The user asked for the command palette: the host runs `ui::keys::choose` over
    /// `App::keys_rows` and passes the pick to `App::run_palette`.
    Palette,
}

enum Mode {
    Normal,
    /// The password of this network.
    Password(String, Input),
    Forget(String),
}

/// A connection attempt, on its own thread (it waits for the network).
struct Job {
    label: String,
    handle: std::thread::JoinHandle<nu_core::Result<()>>,
}

pub struct App {
    backend: Arc<dyn Backend>,
    theme: Theme,
    status: Status,
    networks: Vec<Network>,
    cursor: usize,
    offset: usize,
    mode: Mode,
    message: Option<(String, bool)>,
    keymap: Keymap,
    help: Option<ui::keys::Help>,
    job: Option<Job>,
    refreshed: Instant,
}

impl App {
    pub fn new(backend: Arc<dyn Backend>) -> App {
        let mut app = App {
            backend,
            theme: Theme::load(),
            status: Status::default(),
            networks: Vec::new(),
            cursor: 0,
            offset: 0,
            mode: Mode::Normal,
            message: None,
            keymap: Keymap::new("space", keys::ACTIONS, &[]),
            help: None,
            job: None,
            refreshed: Instant::now(),
        };
        app.refresh();
        app
    }

    /// Reread the status and the networks, keeping the cursor on the same network.
    fn refresh(&mut self) {
        let keep = self.networks.get(self.cursor).map(|n| n.ssid.clone());
        match self.backend.status() {
            Ok(s) => self.status = s,
            Err(e) => self.say(e.to_string(), true),
        }
        match self.backend.networks() {
            Ok(n) => self.networks = n,
            Err(e) => self.say(e.to_string(), true),
        }
        if let Some(ssid) = keep {
            if let Some(i) = self.networks.iter().position(|n| n.ssid == ssid) {
                self.cursor = i;
            }
        }
        self.cursor = self.cursor.min(self.networks.len().saturating_sub(1));
        self.refreshed = Instant::now();
    }

    fn say(&mut self, msg: impl Into<String>, error: bool) {
        self.message = Some((msg.into(), error));
    }

    fn current(&self) -> Option<&Network> {
        self.networks.get(self.cursor)
    }

    /// Call between events: finishes a connection attempt that ended and rereads the list now
    /// and then. `true` when the screen should be redrawn.
    pub fn tick(&mut self) -> bool {
        if self.job.as_ref().is_some_and(|j| j.handle.is_finished()) {
            let job = self.job.take().expect("checked above");
            match job.handle.join() {
                Ok(Ok(())) => self.say(format!("{} done", job.label), false),
                Ok(Err(e)) => self.say(e.to_string(), true),
                Err(_) => self.say("the connection attempt crashed", true),
            }
            self.refresh();
            return true;
        }
        if self.job.is_none() && matches!(self.mode, Mode::Normal) && self.refreshed.elapsed() >= REFRESH {
            self.refresh();
            return true;
        }
        self.job.is_some()
    }

    pub fn handle_event(&mut self, ev: &Event) -> Quit {
        let Event::Key(key @ KeyEvent { kind, .. }) = ev else { return Quit::No };
        if *kind == KeyEventKind::Release {
            return Quit::No;
        }
        if let Some(help) = self.help.as_mut() {
            if !help.key(key) {
                self.help = None;
            }
            return Quit::No;
        }
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Password(ssid, mut input) => {
                match key.code {
                    KeyCode::Esc => {}
                    KeyCode::Enter => {
                        let password = input.value().to_string();
                        self.start_connect(ssid, Some(password));
                    }
                    _ => {
                        input.handle_event(ev);
                        self.mode = Mode::Password(ssid, input);
                    }
                }
                return Quit::No;
            }
            Mode::Forget(ssid) => {
                if matches!(key.code, KeyCode::Char('y' | 'Y') | KeyCode::Enter) {
                    match self.backend.forget(&ssid) {
                        Ok(()) => self.say(format!("forgot {ssid}"), false),
                        Err(e) => self.say(e.to_string(), true),
                    }
                    self.refresh();
                }
                return Quit::No;
            }
            Mode::Normal => {}
        }
        self.message = None;
        match self.keymap.feed(key) {
            Feed::Run(id) => self.run(id),
            Feed::Pending | Feed::Cancelled | Feed::Unbound => Quit::No,
        }
    }

    /// Every action with its keys, for the palette and the help.
    pub fn keys_rows(&self) -> Vec<ui::keys::Row> {
        self.keymap.rows()
    }

    /// Run the action the palette picked.
    pub fn run_palette(&mut self, id: &'static str) -> Quit {
        self.run(id)
    }

    fn start_connect(&mut self, ssid: String, password: Option<String>) {
        if self.job.is_some() {
            self.say("busy: wait for the current connection attempt", true);
            return;
        }
        let backend = self.backend.clone();
        let label = format!("connecting to {ssid}");
        let handle = std::thread::spawn(move || backend.connect(&ssid, password.as_deref()));
        self.job = Some(Job { label, handle });
    }

    fn run(&mut self, id: &'static str) -> Quit {
        let last = self.networks.len().saturating_sub(1);
        match id {
            "nav.up" => self.cursor = self.cursor.saturating_sub(1),
            "nav.down" => self.cursor = (self.cursor + 1).min(last),
            "nav.page_up" => self.cursor = self.cursor.saturating_sub(10),
            "nav.page_down" => self.cursor = (self.cursor + 10).min(last),
            "nav.top" => self.cursor = 0,
            "nav.bottom" => self.cursor = last,
            "wifi.connect" => {
                if let Some(n) = self.current().cloned() {
                    if n.active {
                        return self.run("wifi.disconnect");
                    }
                    // A new protected network needs its password first.
                    if !n.known && !n.security.is_open() && n.security != Security::Enterprise {
                        self.mode = Mode::Password(n.ssid, Input::default());
                    } else {
                        self.start_connect(n.ssid, None);
                    }
                }
            }
            "wifi.disconnect" => match self.backend.disconnect() {
                Ok(()) => {
                    self.say("disconnected", false);
                    self.refresh();
                }
                Err(e) => self.say(e.to_string(), true),
            },
            "wifi.forget" => match self.current() {
                Some(n) if n.known => self.mode = Mode::Forget(n.ssid.clone()),
                Some(_) => self.say("no saved connection for this network", true),
                None => {}
            },
            "wifi.scan" => match self.backend.scan() {
                Ok(()) => {
                    self.say("scanning (the list updates in a few seconds)", false);
                    self.refresh();
                }
                Err(e) => self.say(e.to_string(), true),
            },
            "wifi.power" => {
                let on = !self.status.enabled;
                match self.backend.set_enabled(on) {
                    Ok(()) => {
                        self.say(if on { "Wi-Fi on" } else { "Wi-Fi off" }, false);
                        self.refresh();
                    }
                    Err(e) => self.say(e.to_string(), true),
                }
            }
            "palette" => return Quit::Palette,
            "help" => self.help = Some(ui::keys::Help::default()),
            "quit" => return Quit::Yes,
            _ => {}
        }
        Quit::No
    }

    // ---- drawing --------------------------------------------------------

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        let [list, details] = ui::layout::main_preview(&theme, body);
        self.render_list(list, buf, &theme);
        self.render_details(details, buf, &theme);
        self.render_status(status, buf, &theme);
        if let Some(menu) = self.keymap.menu() {
            ui::keys::render_menu(area, buf, &theme, &menu);
        }
        if self.help.is_some() {
            let rows = self.keymap.rows();
            if let Some(h) = self.help.as_mut() {
                h.render(area, buf, &theme, "Keys", &rows);
            }
        }
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let mut pane = Pane::new(theme).title("Wi-Fi").focused(true);
        if let Some(icon) = ui::icons::for_title("Wi-Fi") {
            pane = pane.icon(icon);
        }
        let inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height == 0 || inner.width < 8 {
            return;
        }
        if self.status.device.is_none() {
            buf.set_string(inner.x, inner.y, "no Wi-Fi device on this machine", Style::new().fg(c.muted.into()));
            return;
        }
        if !self.status.enabled {
            buf.set_string(inner.x, inner.y, "Wi-Fi is off (space p turns it on)", Style::new().fg(c.warning.into()));
            return;
        }
        if self.networks.is_empty() {
            buf.set_string(inner.x, inner.y, "no networks in range (s scans)", Style::new().fg(c.muted.into()));
            return;
        }
        let page = usize::from(inner.height);
        if self.cursor < self.offset {
            self.offset = self.cursor;
        } else if self.cursor >= self.offset + page {
            self.offset = self.cursor + 1 - page;
        }
        let rows = ui::list::Rows::new(area, inner);
        for (row, i) in (self.offset..(self.offset + page).min(self.networks.len())).enumerate() {
            let n = &self.networks[i];
            let y = inner.y + row as u16;
            let base = ui::list::style(theme, i == self.cursor, n.active.then(|| c.success.into()));
            buf.set_style(rows.bar(y), base);
            let content = rows.content(y);
            let name_style = if n.active { Style::new().fg(c.success.into()).add_modifier(Modifier::BOLD) } else { Style::new().fg(c.fg.into()) };
            let left = Line::from(vec![Span::styled(format!("{} ", bars(n.strength)), Style::new().fg(strength_color(theme, n.strength))), Span::styled(n.ssid.clone(), name_style)]);
            let mut tags = String::new();
            if n.known {
                tags.push_str("saved ");
            }
            tags.push_str(n.security.label());
            if n.active {
                tags.push_str(" ●");
            }
            let tw = tags.chars().count() as u16;
            let room = if content.width > tw + 12 { content.width - tw - 1 } else { content.width };
            Paragraph::new(left).style(base).render(Rect { width: room, ..content }, buf);
            if room < content.width {
                buf.set_string(content.right() - tw, y, tags, base.fg(c.muted.into()));
            }
        }
    }

    fn render_details(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let pane = Pane::new(theme).title(self.current().map(|n| n.ssid.as_str()).unwrap_or("Details")).icon(ui::icons::WIFI);
        let inner = pane.inner(area);
        pane.render(area, buf);
        let Some(n) = self.current() else { return };
        let label = |t: &str| Span::styled(format!("{t:<10}"), Style::new().fg(c.muted.into()));
        let val = |t: String| Span::styled(t, Style::new().fg(c.fg.into()));
        let band = if n.frequency_mhz >= 5900 { "6 GHz" } else if n.frequency_mhz >= 4900 { "5 GHz" } else { "2.4 GHz" };
        let lines = vec![
            Line::from(vec![label("Signal"), Span::styled(format!("{} ", bars(n.strength)), Style::new().fg(strength_color(theme, n.strength))), val(format!("{}%", n.strength))]),
            Line::from(vec![label("Security"), val(n.security.label().to_string())]),
            Line::from(vec![label("Band"), val(format!("{band} ({} MHz)", n.frequency_mhz))]),
            Line::from(vec![label("Access pt"), val(n.bssid.clone())]),
            Line::from(vec![label("Saved"), val(if n.known { "yes" } else { "no" }.to_string())]),
            Line::from(vec![label("Connected"), val(if n.active { "yes" } else { "no" }.to_string())]),
        ];
        Paragraph::new(lines).render(inner, buf);
    }

    fn render_status(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let line = match &self.mode {
            Mode::Password(ssid, input) => {
                let label = format!("password for {ssid}: ");
                ui::prompt::cursor(buf, area, &label, input.visual_cursor());
                ui::prompt::line(theme, &label, &"•".repeat(input.value().chars().count()))
            }
            Mode::Forget(ssid) => Line::from(Span::styled(format!("Forget {ssid}? (y/n)"), Style::new().fg(c.warning.into()))),
            Mode::Normal if self.job.is_some() => Line::from(Span::styled(format!("{} …", self.job.as_ref().map(|j| j.label.as_str()).unwrap_or("")), Style::new().fg(c.accent.into()))),
            Mode::Normal => match &self.message {
                Some((m, err)) => Line::from(Span::styled(m.clone(), Style::new().fg(if *err { c.error } else { c.accent }.into()))),
                None => Line::from(Span::styled("enter connect  x disconnect  d forget  s scan  space p Wi-Fi on/off  space space commands  ? keys", Style::new().fg(c.muted.into()))),
            },
        };
        Paragraph::new(line).render(Rect { x: area.x + 1, width: area.width.saturating_sub(2), ..area }, buf);
    }
}

/// Four-step signal bars: `▂▄▆█` filled by strength.
fn bars(strength: u8) -> String {
    let filled = match strength {
        0..=19 => 1,
        20..=44 => 2,
        45..=69 => 3,
        _ => 4,
    };
    ["▂", "▄", "▆", "█"].iter().enumerate().map(|(i, b)| if i < filled { *b } else { " " }).collect()
}

fn strength_color(theme: &Theme, strength: u8) -> ratatui::style::Color {
    let c = &theme.colors;
    match strength {
        0..=29 => c.error,
        30..=54 => c.warning,
        _ => c.success,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nu_core::fake::Fake;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn press(app: &mut App, keys: &str) {
        for c in keys.chars() {
            app.handle_event(&key(KeyCode::Char(c)));
        }
    }

    fn app() -> (App, Arc<Fake>) {
        let fake = Arc::new(Fake::new());
        (App::new(fake.clone()), fake)
    }

    fn screen(app: &mut App) -> String {
        let area = Rect::new(0, 0, 100, 20);
        let mut buf = Buffer::empty(area);
        app.render(area, &mut buf);
        (0..20).map(|y| (0..100).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    }

    fn wait(app: &mut App) {
        for _ in 0..300 {
            if app.job.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
            app.tick();
        }
    }

    #[test]
    fn the_list_and_the_details_show_the_networks() {
        let (mut app, _) = app();
        let s = screen(&mut app);
        for want in ["Wi-Fi", "home", "cafe", "office", "saved", "WPA", "open", "802.1X", "Signal", "Security"] {
            assert!(s.contains(want), "{want} in\n{s}");
        }
        press(&mut app, "j");
        assert!(screen(&mut app).contains("open"), "the details follow the cursor");
    }

    #[test]
    fn enter_connects_asks_for_a_password_when_needed_and_toggles_off() {
        let (mut app, fake) = app();
        // `home` is saved: no password. The attempt runs on a thread.
        app.handle_event(&key(KeyCode::Enter));
        assert!(app.job.is_some());
        wait(&mut app);
        assert_eq!(app.status.active.as_deref(), Some("home"), "{:?}", app.message);
        // Enter on the connected network disconnects.
        app.handle_event(&key(KeyCode::Enter));
        assert_eq!(app.status.active, None);
        // `neighbor` is new and protected: a masked prompt, a short password is refused, a long one works.
        app.cursor = app.networks.iter().position(|n| n.ssid == "neighbor").unwrap();
        app.handle_event(&key(KeyCode::Enter));
        assert!(matches!(app.mode, Mode::Password(..)));
        press(&mut app, "short");
        let masked = screen(&mut app);
        assert!(masked.contains("•••••") && !masked.contains("short"), "{masked}");
        app.handle_event(&key(KeyCode::Enter));
        wait(&mut app);
        assert!(app.message.as_ref().is_some_and(|(_, e)| *e), "{:?}", app.message);
        app.cursor = app.networks.iter().position(|n| n.ssid == "neighbor").unwrap();
        app.handle_event(&key(KeyCode::Enter));
        press(&mut app, "longenoughpw");
        app.handle_event(&key(KeyCode::Enter));
        wait(&mut app);
        assert_eq!(app.status.active.as_deref(), Some("neighbor"), "{:?}", app.message);
        assert!(fake.log().iter().any(|l| l == "connect neighbor"));
        // Esc cancels a prompt.
        app.cursor = app.networks.iter().position(|n| n.ssid == "cafe").unwrap();
        app.handle_event(&key(KeyCode::Enter));
        wait(&mut app); // cafe is open: connects at once
        assert_eq!(app.status.active.as_deref(), Some("cafe"));
    }

    #[test]
    fn forget_asks_first_and_scan_and_power_work() {
        let (mut app, _) = app();
        press(&mut app, "d");
        assert!(matches!(app.mode, Mode::Forget(_)));
        press(&mut app, "n");
        assert!(app.networks.iter().any(|n| n.ssid == "home" && n.known));
        press(&mut app, "d");
        press(&mut app, "y");
        assert!(app.networks.iter().any(|n| n.ssid == "home" && !n.known));
        // Forgetting a network that is not saved says so.
        press(&mut app, "d");
        assert!(matches!(app.mode, Mode::Normal) && app.message.as_ref().is_some_and(|(_, e)| *e));
        press(&mut app, "s");
        assert!(app.message.as_ref().is_some_and(|(m, _)| m.contains("scanning")));
        // Space p turns the radio off: the list says so.
        press(&mut app, " ");
        press(&mut app, "p");
        assert!(!app.status.enabled);
        assert!(screen(&mut app).contains("Wi-Fi is off"));
        press(&mut app, " ");
        press(&mut app, "p");
        assert!(app.status.enabled && app.networks.len() == 4);
    }

    #[test]
    fn leader_menu_palette_help_and_quit() {
        let (mut app, _) = app();
        press(&mut app, " ");
        let menu = app.keymap.menu().unwrap();
        let names: Vec<&str> = menu.entries.iter().map(|e| e.name.as_str()).collect();
        for want in ["connect", "disconnect", "forget", "scan", "toggle Wi-Fi", "commands"] {
            assert!(names.contains(&want), "{want} in {names:?}");
        }
        assert_eq!(app.handle_event(&key(KeyCode::Char(' '))), Quit::Palette);
        assert_eq!(app.run_palette("wifi.scan"), Quit::No);
        press(&mut app, "?");
        assert!(app.help.is_some());
        app.handle_event(&key(KeyCode::Esc));
        assert!(app.help.is_none());
        assert_eq!(app.handle_event(&key(KeyCode::Char('q'))), Quit::Yes);
    }

    #[test]
    fn signal_bars_follow_the_strength() {
        assert_eq!(bars(10), "▂   ");
        assert_eq!(bars(50), "▂▄▆ ");
        assert_eq!(bars(95), "▂▄▆█");
    }
}
