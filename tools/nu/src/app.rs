use crate::keys;
use nu_core::bluetooth::{Bluetooth, BtStatus, Device, rssi_percent};
use nu_core::{Backend, Network, Security, Status};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;
use ui::keys::{Feed, Keymap};
use ui::{Pane, Theme};

/// How often the lists are reread while the screen is open.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Wifi,
    Bluetooth,
}

const ORDER: [Focus; 2] = [Focus::Wifi, Focus::Bluetooth];

enum Mode {
    Normal,
    /// The password of this network.
    Password(String, Input),
    /// Forget this Wi-Fi network.
    ForgetWifi(String),
    /// Forget this Bluetooth device (address and name).
    ForgetDevice(String, String),
}

/// A connection attempt, on its own thread (it waits for the network or the device).
struct Job {
    label: String,
    handle: std::thread::JoinHandle<nu_core::Result<()>>,
}

#[derive(Default, Clone, Copy)]
struct ListState {
    cursor: usize,
    offset: usize,
}

pub struct App {
    wifi: Arc<dyn Backend>,
    bt: Arc<dyn Bluetooth>,
    theme: Theme,
    wifi_status: Status,
    networks: Vec<Network>,
    /// Why Wi-Fi is unavailable (no NetworkManager, no D-Bus), if it is.
    wifi_error: Option<String>,
    bt_status: BtStatus,
    devices: Vec<Device>,
    bt_error: Option<String>,
    lists: [ListState; 2],
    focus: Focus,
    mode: Mode,
    message: Option<(String, bool)>,
    maps: [Keymap; 2],
    help: Option<ui::keys::Help>,
    job: Option<Job>,
    refreshed: Instant,
}

impl App {
    pub fn new(wifi: Arc<dyn Backend>, bt: Arc<dyn Bluetooth>) -> App {
        let mut app = App {
            wifi,
            bt,
            theme: Theme::load(),
            wifi_status: Status::default(),
            networks: Vec::new(),
            wifi_error: None,
            bt_status: BtStatus::default(),
            devices: Vec::new(),
            bt_error: None,
            lists: [ListState::default(); 2],
            focus: Focus::Wifi,
            mode: Mode::Normal,
            message: None,
            maps: [0, 1].map(|i| Keymap::new("space", &keys::pane_actions(i), &[])),
            help: None,
            job: None,
            refreshed: Instant::now(),
        };
        app.refresh();
        app
    }

    fn pane(focus: Focus) -> usize {
        match focus {
            Focus::Wifi => 0,
            Focus::Bluetooth => 1,
        }
    }

    fn len(&self, focus: Focus) -> usize {
        match focus {
            Focus::Wifi => self.networks.len(),
            Focus::Bluetooth => self.devices.len(),
        }
    }

    /// Reread both radios, keeping each cursor on the same entry.
    fn refresh(&mut self) {
        let keep_wifi = self.networks.get(self.lists[0].cursor).map(|n| n.ssid.clone());
        match self.wifi.status().and_then(|s| self.wifi.networks().map(|n| (s, n))) {
            Ok((s, n)) => {
                self.wifi_status = s;
                self.networks = n;
                self.wifi_error = None;
            }
            Err(e) => {
                self.wifi_status = Status::default();
                self.networks.clear();
                self.wifi_error = Some(e.to_string());
            }
        }
        if let Some(i) = keep_wifi.and_then(|ssid| self.networks.iter().position(|n| n.ssid == ssid)) {
            self.lists[0].cursor = i;
        }
        let keep_bt = self.devices.get(self.lists[1].cursor).map(|d| d.address.clone());
        match self.bt.status().and_then(|s| self.bt.devices().map(|d| (s, d))) {
            Ok((s, d)) => {
                self.bt_status = s;
                self.devices = d;
                self.bt_error = None;
            }
            Err(e) => {
                self.bt_status = BtStatus::default();
                self.devices.clear();
                self.bt_error = Some(e.to_string());
            }
        }
        if let Some(i) = keep_bt.and_then(|a| self.devices.iter().position(|d| d.address == a)) {
            self.lists[1].cursor = i;
        }
        for (i, f) in ORDER.into_iter().enumerate() {
            self.lists[i].cursor = self.lists[i].cursor.min(self.len(f).saturating_sub(1));
        }
        self.refreshed = Instant::now();
    }

    fn say(&mut self, msg: impl Into<String>, error: bool) {
        self.message = Some((msg.into(), error));
    }

    fn network(&self) -> Option<&Network> {
        self.networks.get(self.lists[0].cursor)
    }

    fn device(&self) -> Option<&Device> {
        self.devices.get(self.lists[1].cursor)
    }

    /// Call between events: finishes a connection attempt that ended and rereads the lists now
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
                        self.start_wifi(ssid, Some(password));
                    }
                    _ => {
                        input.handle_event(ev);
                        self.mode = Mode::Password(ssid, input);
                    }
                }
                return Quit::No;
            }
            Mode::ForgetWifi(ssid) => {
                if matches!(key.code, KeyCode::Char('y' | 'Y') | KeyCode::Enter) {
                    match self.wifi.forget(&ssid) {
                        Ok(()) => self.say(format!("forgot {ssid}"), false),
                        Err(e) => self.say(e.to_string(), true),
                    }
                    self.refresh();
                }
                return Quit::No;
            }
            Mode::ForgetDevice(address, name) => {
                if matches!(key.code, KeyCode::Char('y' | 'Y') | KeyCode::Enter) {
                    match self.bt.remove(&address) {
                        Ok(()) => self.say(format!("forgot {name}"), false),
                        Err(e) => self.say(e.to_string(), true),
                    }
                    self.refresh();
                }
                return Quit::No;
            }
            Mode::Normal => {}
        }
        self.message = None;
        let pane = Self::pane(self.focus);
        match self.maps[pane].feed(key) {
            Feed::Run(id) => self.run(id),
            Feed::NoBinding => {
                let seq = self.maps[pane].rejected().to_string();
                self.say(format!("{seq} is not bound"), true);
                Quit::No
            }
            Feed::Pending | Feed::Cancelled | Feed::Unbound => Quit::No,
        }
    }

    /// Every action of the focused pane with its keys, for the palette and the help.
    pub fn keys_rows(&self) -> Vec<ui::keys::Row> {
        self.maps[Self::pane(self.focus)].rows()
    }

    /// Run the action the palette picked.
    pub fn run_palette(&mut self, id: &'static str) -> Quit {
        self.run(id)
    }

    fn busy(&mut self) -> bool {
        if self.job.is_some() {
            self.say("busy: wait for the current connection attempt", true);
            return true;
        }
        false
    }

    fn start_wifi(&mut self, ssid: String, password: Option<String>) {
        if self.busy() {
            return;
        }
        let backend = self.wifi.clone();
        let label = format!("connecting to {ssid}");
        let handle = std::thread::spawn(move || backend.connect(&ssid, password.as_deref()));
        self.job = Some(Job { label, handle });
    }

    fn start_bluetooth(&mut self, device: &Device) {
        if self.busy() {
            return;
        }
        let backend = self.bt.clone();
        let (address, label) = (device.address.clone(), if device.paired { format!("connecting to {}", device.name) } else { format!("pairing with {}", device.name) });
        let handle = std::thread::spawn(move || backend.connect(&address));
        self.job = Some(Job { label, handle });
    }

    fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
    }

    fn focus_by(&mut self, delta: isize) {
        let at = ORDER.iter().position(|f| *f == self.focus).unwrap_or(0) as isize;
        self.focus = ORDER[(at + delta).rem_euclid(ORDER.len() as isize) as usize];
    }

    fn run(&mut self, id: &'static str) -> Quit {
        let focus = self.focus;
        let last = self.len(focus).saturating_sub(1);
        let st = &mut self.lists[Self::pane(focus)];
        match id {
            "nav.up" => st.cursor = st.cursor.saturating_sub(1),
            "nav.down" => st.cursor = (st.cursor + 1).min(last),
            "nav.up5" => st.cursor = st.cursor.saturating_sub(5),
            "nav.down5" => st.cursor = (st.cursor + 5).min(last),
            "nav.page_up" => st.cursor = st.cursor.saturating_sub(10),
            "nav.page_down" => st.cursor = (st.cursor + 10).min(last),
            "nav.top" => st.cursor = 0,
            "nav.bottom" => st.cursor = last,
            "focus.next" => self.focus_by(1),
            "focus.prev" => self.focus_by(-1),
            "focus.wifi" => self.set_focus(Focus::Wifi),
            "focus.bluetooth" => self.set_focus(Focus::Bluetooth),
            "palette" => return Quit::Palette,
            "help" => self.help = Some(ui::keys::Help::default()),
            "quit" => return Quit::Yes,
            _ => match focus {
                Focus::Wifi => self.wifi_action(id),
                Focus::Bluetooth => self.bluetooth_action(id),
            },
        }
        Quit::No
    }

    fn wifi_action(&mut self, id: &str) {
        match id {
            "wifi.connect" => {
                if let Some(n) = self.network().cloned() {
                    if n.active {
                        return self.wifi_action("wifi.disconnect");
                    }
                    // A new protected network needs its password first.
                    if !n.known && !n.security.is_open() && n.security != Security::Enterprise {
                        self.mode = Mode::Password(n.ssid, Input::default());
                    } else {
                        self.start_wifi(n.ssid, None);
                    }
                }
            }
            "wifi.disconnect" => match self.wifi.disconnect() {
                Ok(()) => {
                    self.say("disconnected", false);
                    self.refresh();
                }
                Err(e) => self.say(e.to_string(), true),
            },
            "wifi.forget" => match self.network() {
                Some(n) if n.known => self.mode = Mode::ForgetWifi(n.ssid.clone()),
                Some(_) => self.say("no saved connection for this network", true),
                None => {}
            },
            "wifi.scan" => match self.wifi.scan() {
                Ok(()) => {
                    self.say("scanning (the list updates in a few seconds)", false);
                    self.refresh();
                }
                Err(e) => self.say(e.to_string(), true),
            },
            "wifi.power" => {
                let on = !self.wifi_status.enabled;
                match self.wifi.set_enabled(on) {
                    Ok(()) => {
                        self.say(if on { "Wi-Fi on" } else { "Wi-Fi off" }, false);
                        self.refresh();
                    }
                    Err(e) => self.say(e.to_string(), true),
                }
            }
            _ => {}
        }
    }

    fn bluetooth_action(&mut self, id: &str) {
        match id {
            "bt.connect" => {
                if let Some(d) = self.device().cloned() {
                    if d.connected {
                        return self.bluetooth_action("bt.disconnect");
                    }
                    self.start_bluetooth(&d);
                }
            }
            "bt.disconnect" => {
                let Some(d) = self.device().cloned() else { return };
                match self.bt.disconnect(&d.address) {
                    Ok(()) => {
                        self.say(format!("disconnected {}", d.name), false);
                        self.refresh();
                    }
                    Err(e) => self.say(e.to_string(), true),
                }
            }
            "bt.remove" => {
                if let Some(d) = self.device() {
                    self.mode = Mode::ForgetDevice(d.address.clone(), d.name.clone());
                }
            }
            "bt.trust" => {
                let Some(d) = self.device().cloned() else { return };
                match self.bt.set_trusted(&d.address, !d.trusted) {
                    Ok(()) => {
                        self.say(if d.trusted { format!("{} is no longer trusted", d.name) } else { format!("trusting {}", d.name) }, false);
                        self.refresh();
                    }
                    Err(e) => self.say(e.to_string(), true),
                }
            }
            "bt.scan" => {
                let on = !self.bt_status.discovering;
                match self.bt.scan(on) {
                    Ok(()) => {
                        self.say(if on { "looking for devices" } else { "stopped looking" }, false);
                        self.refresh();
                    }
                    Err(e) => self.say(e.to_string(), true),
                }
            }
            "bt.power" => {
                let on = !self.bt_status.powered;
                match self.bt.set_powered(on) {
                    Ok(()) => {
                        self.say(if on { "Bluetooth on" } else { "Bluetooth off" }, false);
                        self.refresh();
                    }
                    Err(e) => self.say(e.to_string(), true),
                }
            }
            _ => {}
        }
    }

    // ---- drawing --------------------------------------------------------

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        let [left, right] = ui::layout::main_preview(&theme, body);
        // The focused pane takes the room; the other shows a few rows.
        let constraints = ORDER.map(|f| if f == self.focus { Constraint::Min(7) } else { Constraint::Length(6) });
        let panes = Layout::vertical(constraints).split(left);
        self.render_wifi(panes[0], buf, &theme);
        self.render_bluetooth(panes[1], buf, &theme);
        match self.focus {
            Focus::Wifi => self.details_wifi(right, buf, &theme),
            Focus::Bluetooth => self.details_bluetooth(right, buf, &theme),
        }
        self.render_status(status, buf, &theme);
        if let Some(menu) = self.maps[Self::pane(self.focus)].menu() {
            ui::keys::render_menu(area, buf, &theme, &menu);
        }
        if self.help.is_some() {
            let rows = self.keys_rows();
            if let Some(h) = self.help.as_mut() {
                h.render(area, buf, &theme, "Keys", &rows);
            }
        }
    }

    /// A pane with the list `rows` drawn in it: each entry is `(left, right, highlighted tint)`.
    fn render_list(&mut self, focus: Focus, title: &str, area: Rect, buf: &mut Buffer, theme: &Theme, note: Option<(String, bool)>, rows: Vec<(Line<'static>, String, bool)>) {
        let c = &theme.colors;
        let focused = self.focus == focus;
        let mut pane = Pane::new(theme).title(title).focused(focused);
        if let Some(icon) = ui::icons::for_title(title) {
            pane = pane.icon(icon);
        }
        let inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height == 0 || inner.width < 8 {
            return;
        }
        if let Some((text, warn)) = note {
            // A reason can be long: wrap it inside the pane.
            Paragraph::new(text).wrap(ratatui::widgets::Wrap { trim: true }).style(Style::new().fg(if warn { c.warning } else { c.muted }.into())).render(inner, buf);
            return;
        }
        let page = usize::from(inner.height);
        let st = &mut self.lists[Self::pane(focus)];
        if st.cursor < st.offset {
            st.offset = st.cursor;
        } else if st.cursor >= st.offset + page {
            st.offset = st.cursor + 1 - page;
        }
        let (cursor, offset) = (st.cursor, st.offset);
        let geo = ui::list::Rows::new(area, inner);
        for (row, i) in (offset..(offset + page).min(rows.len())).enumerate() {
            let (left, right, active) = &rows[i];
            let y = inner.y + row as u16;
            let base = ui::list::style(theme, i == cursor && focused, active.then(|| c.success.into()));
            buf.set_style(geo.bar(y), base);
            let content = geo.content(y);
            let tw = right.chars().count() as u16;
            let room = if content.width > tw + 12 { content.width - tw - 1 } else { content.width };
            Paragraph::new(left.clone()).style(base).render(Rect { width: room, ..content }, buf);
            if room < content.width {
                buf.set_string(content.right() - tw, y, right, base.fg(c.muted.into()));
            }
        }
    }

    fn render_wifi(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let note = if let Some(e) = &self.wifi_error {
            Some((format!("unavailable: {e}"), false))
        } else if self.wifi_status.device.is_none() {
            Some(("no Wi-Fi device on this machine".to_string(), false))
        } else if !self.wifi_status.enabled {
            Some(("Wi-Fi is off (space p turns it on)".to_string(), true))
        } else if self.networks.is_empty() {
            Some(("no networks in range (s scans)".to_string(), false))
        } else {
            None
        };
        let rows: Vec<(Line<'static>, String, bool)> = self
            .networks
            .iter()
            .map(|n| {
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
                (left, tags, n.active)
            })
            .collect();
        self.render_list(Focus::Wifi, "Wi-Fi", area, buf, theme, note, rows);
    }

    fn render_bluetooth(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let note = if let Some(e) = &self.bt_error {
            Some((format!("unavailable: {e}"), false))
        } else if self.bt_status.adapter.is_none() {
            Some(("no Bluetooth adapter on this machine".to_string(), false))
        } else if !self.bt_status.powered {
            Some(("Bluetooth is off (space p turns it on)".to_string(), true))
        } else if self.devices.is_empty() {
            Some(("no devices (s looks for some)".to_string(), false))
        } else {
            None
        };
        let rows: Vec<(Line<'static>, String, bool)> = self
            .devices
            .iter()
            .map(|d| {
                let name_style = if d.connected { Style::new().fg(c.success.into()).add_modifier(Modifier::BOLD) } else if d.paired { Style::new().fg(c.fg.into()) } else { Style::new().fg(c.muted.into()) };
                let signal = d.rssi.map(rssi_percent);
                let left = Line::from(vec![
                    match signal {
                        Some(s) => Span::styled(format!("{} ", bars(s)), Style::new().fg(strength_color(theme, s))),
                        None => Span::styled("     ", Style::new()),
                    },
                    Span::styled(d.name.clone(), name_style),
                ]);
                let mut tags = String::from(d.kind.label());
                if d.paired {
                    tags.push_str(" paired");
                }
                if d.connected {
                    tags.push_str(" ●");
                }
                (left, tags, d.connected)
            })
            .collect();
        let title = if self.bt_status.discovering { "Bluetooth (scanning)" } else { "Bluetooth" };
        // The title keeps its icon by name; the scanning note is appended to the title text only.
        self.render_list(Focus::Bluetooth, title, area, buf, theme, note, rows);
    }

    fn details_wifi(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let pane = Pane::new(theme).title(self.network().map(|n| n.ssid.as_str()).unwrap_or("Details")).icon(ui::icons::WIFI);
        let inner = pane.inner(area);
        pane.render(area, buf);
        let Some(n) = self.network() else { return };
        let label = |t: &str| Span::styled(format!("{t:<10}"), Style::new().fg(c.muted.into()));
        let val = |t: String| Span::styled(t, Style::new().fg(c.fg.into()));
        let band = if n.frequency_mhz >= 5900 { "6 GHz" } else if n.frequency_mhz >= 4900 { "5 GHz" } else { "2.4 GHz" };
        let lines = vec![
            Line::from(vec![label("Signal"), Span::styled(format!("{} ", bars(n.strength)), Style::new().fg(strength_color(theme, n.strength))), val(format!("{}%", n.strength))]),
            Line::from(vec![label("Security"), val(n.security.label().to_string())]),
            Line::from(vec![label("Band"), val(format!("{band} ({} MHz)", n.frequency_mhz))]),
            Line::from(vec![label("Access pt"), val(n.bssid.clone())]),
            Line::from(vec![label("Saved"), val(yes(n.known))]),
            Line::from(vec![label("Connected"), val(yes(n.active))]),
        ];
        Paragraph::new(lines).render(inner, buf);
    }

    fn details_bluetooth(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let pane = Pane::new(theme).title(self.device().map(|d| d.name.as_str()).unwrap_or("Details")).icon(ui::icons::BLUETOOTH);
        let inner = pane.inner(area);
        pane.render(area, buf);
        let Some(d) = self.device() else { return };
        let label = |t: &str| Span::styled(format!("{t:<10}"), Style::new().fg(c.muted.into()));
        let val = |t: String| Span::styled(t, Style::new().fg(c.fg.into()));
        let signal = match d.rssi {
            Some(r) => {
                let s = rssi_percent(r);
                Line::from(vec![label("Signal"), Span::styled(format!("{} ", bars(s)), Style::new().fg(strength_color(theme, s))), val(format!("{r} dBm"))])
            }
            None => Line::from(vec![label("Signal"), Span::styled("not in range", Style::new().fg(c.muted.into()))]),
        };
        let lines = vec![
            Line::from(vec![label("Type"), val(d.kind.label().to_string())]),
            Line::from(vec![label("Address"), val(d.address.clone())]),
            signal,
            Line::from(vec![label("Paired"), val(yes(d.paired))]),
            Line::from(vec![label("Trusted"), val(yes(d.trusted))]),
            Line::from(vec![label("Connected"), val(yes(d.connected))]),
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
            Mode::ForgetWifi(ssid) => Line::from(Span::styled(format!("Forget {ssid}? (y/n)"), Style::new().fg(c.warning.into()))),
            Mode::ForgetDevice(_, name) => Line::from(Span::styled(format!("Forget {name}? (y/n)"), Style::new().fg(c.warning.into()))),
            Mode::Normal if self.job.is_some() => Line::from(Span::styled(format!("{} …", self.job.as_ref().map(|j| j.label.as_str()).unwrap_or("")), Style::new().fg(c.accent.into()))),
            Mode::Normal => match &self.message {
                Some((m, err)) => Line::from(Span::styled(m.clone(), Style::new().fg(if *err { c.error } else { c.accent }.into()))),
                None => Line::from(Span::styled(self.hints(), Style::new().fg(c.muted.into()))),
            },
        };
        Paragraph::new(line).render(Rect { x: area.x + 1, width: area.width.saturating_sub(2), ..area }, buf);
    }

    fn hints(&self) -> &'static str {
        match self.focus {
            Focus::Wifi => "enter connect  x disconnect  d forget  s scan  space p on/off  tab pane  space space commands  ? keys",
            Focus::Bluetooth => "enter connect  x disconnect  t trust  d forget  s scan  space p on/off  tab pane  space space commands  ? keys",
        }
    }
}

fn yes(b: bool) -> String {
    if b { "yes" } else { "no" }.to_string()
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
    use nu_core::bluetooth::Kind;
    use nu_core::fake::{Fake, FakeBluetooth};
    use ratatui::crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn press(app: &mut App, keys: &str) {
        for c in keys.chars() {
            app.handle_event(&key(KeyCode::Char(c)));
        }
    }

    fn app() -> (App, Arc<Fake>, Arc<FakeBluetooth>) {
        let (w, b) = (Arc::new(Fake::new()), Arc::new(FakeBluetooth::new()));
        (App::new(w.clone(), b.clone()), w, b)
    }

    fn screen(app: &mut App) -> String {
        let area = Rect::new(0, 0, 110, 26);
        let mut buf = Buffer::empty(area);
        app.render(area, &mut buf);
        (0..26).map(|y| (0..110).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
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
    fn both_panes_and_the_details_show() {
        let (mut app, ..) = app();
        let s = screen(&mut app);
        for want in ["Wi-Fi", "Bluetooth", "home", "cafe", "saved", "WPA", "Headphones", "Keyboard", "audio", "paired", "Signal", "Security"] {
            assert!(s.contains(want), "{want} in\n{s}");
        }
        // The Bluetooth pane's details follow once it has the focus.
        press(&mut app, "2");
        let s = screen(&mut app);
        assert!(s.contains("Address") && s.contains("Trusted") && s.contains("dBm"), "{s}");
    }

    #[test]
    fn wifi_enter_connects_asks_for_a_password_when_needed_and_toggles_off() {
        let (mut app, _, _) = app();
        app.handle_event(&key(KeyCode::Enter)); // `home` is saved: no password
        wait(&mut app);
        assert_eq!(app.wifi_status.active.as_deref(), Some("home"), "{:?}", app.message);
        app.handle_event(&key(KeyCode::Enter)); // on the connected network: disconnect
        assert_eq!(app.wifi_status.active, None);
        app.lists[0].cursor = app.networks.iter().position(|n| n.ssid == "neighbor").unwrap();
        app.handle_event(&key(KeyCode::Enter));
        assert!(matches!(app.mode, Mode::Password(..)));
        press(&mut app, "short");
        let masked = screen(&mut app);
        assert!(masked.contains("•••••") && !masked.contains("short"), "{masked}");
        app.handle_event(&key(KeyCode::Enter));
        wait(&mut app);
        assert!(app.message.as_ref().is_some_and(|(_, e)| *e), "{:?}", app.message);
        app.lists[0].cursor = app.networks.iter().position(|n| n.ssid == "neighbor").unwrap();
        app.handle_event(&key(KeyCode::Enter));
        press(&mut app, "longenoughpw");
        app.handle_event(&key(KeyCode::Enter));
        wait(&mut app);
        assert_eq!(app.wifi_status.active.as_deref(), Some("neighbor"), "{:?}", app.message);
    }

    #[test]
    fn wifi_forget_asks_first_and_scan_and_power_work() {
        let (mut app, ..) = app();
        press(&mut app, "d");
        assert!(matches!(app.mode, Mode::ForgetWifi(_)));
        press(&mut app, "n");
        assert!(app.networks.iter().any(|n| n.ssid == "home" && n.known));
        press(&mut app, "d");
        press(&mut app, "y");
        assert!(app.networks.iter().any(|n| n.ssid == "home" && !n.known));
        press(&mut app, "d");
        assert!(matches!(app.mode, Mode::Normal) && app.message.as_ref().is_some_and(|(_, e)| *e));
        press(&mut app, "s");
        assert!(app.message.as_ref().is_some_and(|(m, _)| m.contains("scanning")));
        press(&mut app, " ");
        press(&mut app, "p");
        assert!(!app.wifi_status.enabled && screen(&mut app).contains("Wi-Fi is off"));
        press(&mut app, " ");
        press(&mut app, "p");
        assert!(app.wifi_status.enabled && app.networks.len() == 4);
    }

    #[test]
    fn bluetooth_connect_pairs_new_devices_toggles_disconnect_trust_forget_scan_and_power() {
        let (mut app, _, bt) = app();
        press(&mut app, "2");
        let at = |app: &App, name: &str| app.devices.iter().position(|d| d.name == name).unwrap();
        // A new device is paired and connected in the background.
        app.lists[1].cursor = at(&app, "Phone");
        app.handle_event(&key(KeyCode::Enter));
        assert!(app.job.as_ref().is_some_and(|j| j.label.contains("pairing")));
        wait(&mut app);
        let phone = app.devices.iter().find(|d| d.name == "Phone").unwrap();
        assert!(phone.paired && phone.trusted && phone.connected, "{:?}", app.message);
        // Enter on a connected device disconnects it.
        app.lists[1].cursor = at(&app, "Phone");
        app.handle_event(&key(KeyCode::Enter));
        assert!(!app.devices.iter().find(|d| d.name == "Phone").unwrap().connected);
        // Trust toggles; a device out of range cannot connect.
        press(&mut app, "t");
        assert!(!app.devices.iter().find(|d| d.name == "Phone").unwrap().trusted);
        app.lists[1].cursor = at(&app, "Old mouse");
        app.handle_event(&key(KeyCode::Enter));
        wait(&mut app);
        assert!(app.message.as_ref().is_some_and(|(m, e)| *e && m.contains("not in range")), "{:?}", app.message);
        // Forget asks first.
        press(&mut app, "d");
        assert!(matches!(app.mode, Mode::ForgetDevice(..)));
        press(&mut app, "y");
        assert!(app.devices.iter().all(|d| d.name != "Old mouse"));
        // Scan starts and stops discovery (the title says so); power turns the adapter off.
        press(&mut app, "s");
        assert!(app.bt_status.discovering && screen(&mut app).contains("scanning"));
        press(&mut app, "s");
        assert!(!app.bt_status.discovering);
        press(&mut app, " ");
        press(&mut app, "p");
        assert!(!app.bt_status.powered && screen(&mut app).contains("Bluetooth is off"));
        assert!(bt.log().iter().any(|l| l == "powered false"));
    }

    #[test]
    fn a_missing_backend_shows_why_in_its_pane() {
        let app_with = |w: Arc<dyn Backend>, b: Arc<dyn Bluetooth>| {
            let mut app = App::new(w, b);
            screen(&mut app)
        };
        let s = app_with(Arc::new(nu_core::Unavailable("NetworkManager is not running".into())), Arc::new(FakeBluetooth::new()));
        assert!(s.contains("unavailable: NetworkManager is not") && s.contains("running") && s.contains("Headphones"), "{s}");
        let s = app_with(Arc::new(Fake::new()), Arc::new(nu_core::Unavailable("BlueZ is not running".into())));
        assert!(s.contains("unavailable: BlueZ is not") && s.contains("running") && s.contains("home"), "{s}");
    }

    #[test]
    fn focus_leader_menu_palette_help_and_quit() {
        let (mut app, ..) = app();
        press(&mut app, "2");
        assert_eq!(app.focus, Focus::Bluetooth);
        app.handle_event(&key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Wifi);
        press(&mut app, " ");
        let menu = app.maps[0].menu().unwrap();
        let names: Vec<&str> = menu.entries.iter().map(|e| e.name.as_str()).collect();
        for want in ["connect", "disconnect", "forget", "scan", "toggle Wi-Fi", "commands"] {
            assert!(names.contains(&want), "{want} in {names:?}");
        }
        assert_eq!(app.handle_event(&key(KeyCode::Char(' '))), Quit::Palette);
        // The palette lists the focused pane's actions.
        let ids: Vec<&str> = app.keys_rows().iter().map(|r| r.id).collect();
        assert!(ids.contains(&"wifi.scan") && !ids.contains(&"bt.scan"), "{ids:?}");
        press(&mut app, "2");
        let ids: Vec<&str> = app.keys_rows().iter().map(|r| r.id).collect();
        assert!(ids.contains(&"bt.trust") && !ids.contains(&"wifi.scan"), "{ids:?}");
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
        assert_eq!(Kind::Audio.label(), "audio");
    }
}
