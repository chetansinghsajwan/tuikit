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
                if matches!(key.code, KeyCode::Char('y' | 'Y' | 'd' | 'D') | KeyCode::Enter) {
                    match self.wifi.forget(&ssid) {
                        Ok(()) => self.say(format!("forgot {ssid}"), false),
                        Err(e) => self.say(e.to_string(), true),
                    }
                    self.refresh();
                }
                return Quit::No;
            }
            Mode::ForgetDevice(address, name) => {
                if matches!(key.code, KeyCode::Char('y' | 'Y' | 'd' | 'D') | KeyCode::Enter) {
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
}

mod render;
#[cfg(test)]
use render::*;

#[cfg(test)]
mod tests;
