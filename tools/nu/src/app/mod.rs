//! The `nu` screen: a Wi-Fi pane and a Bluetooth pane over the `nu-core` backends, with the state,
//! key handling and background connection jobs. Drawing lives in `render`.

use nu_core::bluetooth::{Bluetooth, BtStatus, Device, rssi_percent};
use nu_core::{Backend, Network, Security, Status};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::Event;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tui_input::Input;
use ui::app::{Host, Navigable, Request};
use ui::footer::Footer;
use ui::prompt::Typed;
use ui::{Pane, Theme};

/// How often the lists are reread while the screen is open.
const REFRESH: Duration = Duration::from_secs(4);

/// Which pane has the focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Wifi,
    Bluetooth,
}

/// The panes in the order `tab` visits them (and top to bottom on screen).
const ORDER: [Focus; 2] = [Focus::Wifi, Focus::Bluetooth];

/// What the keys mean right now: normal browsing, or answering a prompt.
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
type Job = ui::job::Job<nu_core::Result<()>>;

use ui::list::ListState;

/// The whole screen: both radios, their lists, the focus and any prompt or running job.
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
    message: Option<ui::footer::Notice>,
    job: Option<Job>,
    refreshed: Instant,
}

impl App {
    /// Builds the screen over two backends and reads them once, so it opens with data.
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
            job: None,
            refreshed: Instant::now(),
        };
        app.refresh();
        app
    }

    /// The index of a pane, for the per-pane `lists` and keymaps.
    fn pane(focus: Focus) -> usize {
        match focus {
            Focus::Wifi => 0,
            Focus::Bluetooth => 1,
        }
    }

    /// How many entries the pane has.
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
        if let Some(i) =
            keep_wifi.and_then(|ssid| self.networks.iter().position(|n| n.ssid == ssid))
        {
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

    /// Shows a message on the status line until the next key; `error` colors it as an error.
    fn show_message(&mut self, msg: impl Into<String>, error: bool) {
        self.message = Some(ui::footer::Notice::new(msg, error));
    }

    /// The Wi-Fi network under the cursor, if any.
    fn network(&self) -> Option<&Network> {
        self.networks.get(self.lists[0].cursor)
    }

    /// The Bluetooth device under the cursor, if any.
    fn device(&self) -> Option<&Device> {
        self.devices.get(self.lists[1].cursor)
    }

    /// Call between events: finishes a connection attempt that ended and rereads the lists now
    /// and then. `true` when the screen should be redrawn.
    pub fn tick(&mut self) -> bool {
        if let Some(done) = ui::job::finished(&mut self.job) {
            match done.result {
                Some(Ok(())) => self.show_message(done.done_text, false),
                Some(Err(e)) => self.show_message(e.to_string(), true),
                None => self.show_message("the connection attempt crashed", true),
            }
            self.refresh();
            return true;
        }
        if self.job.is_none()
            && matches!(self.mode, Mode::Normal)
            && self.refreshed.elapsed() >= REFRESH
        {
            self.refresh();
            return true;
        }
        self.job.is_some()
    }

    /// Handles a key for the open prompt or question; in normal mode it does nothing.
    fn typing(&mut self, ev: &Event) {
        let Event::Key(key) = ev else { return };
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Password(ssid, mut input) => match ui::prompt::typed(&mut input, ev) {
                Typed::Cancel => {}
                Typed::Submit(password) => self.start_wifi(ssid, Some(password)),
                Typed::Editing => self.mode = Mode::Password(ssid, input),
            },
            Mode::ForgetWifi(ssid) => {
                if ui::prompt::confirms(key.code) {
                    match self.wifi.forget(&ssid) {
                        Ok(()) => self.show_message(format!("forgot {ssid}"), false),
                        Err(e) => self.show_message(e.to_string(), true),
                    }
                    self.refresh();
                }
            }
            Mode::ForgetDevice(address, name) => {
                if ui::prompt::confirms(key.code) {
                    match self.bt.remove(&address) {
                        Ok(()) => self.show_message(format!("forgot {name}"), false),
                        Err(e) => self.show_message(e.to_string(), true),
                    }
                    self.refresh();
                }
            }
            Mode::Normal => {}
        }
    }

    /// Whether a connection attempt is already running; says so, since only one runs at a time.
    fn busy(&mut self) -> bool {
        if self.job.is_some() {
            self.show_message("busy: wait for the current connection attempt", true);
            return true;
        }
        false
    }

    /// Connects to a network on a thread, so the screen stays alive while it waits.
    fn start_wifi(&mut self, ssid: String, password: Option<String>) {
        if self.busy() {
            return;
        }
        let backend = self.wifi.clone();
        let label = format!("connecting to {ssid}");
        let done = format!("{label} done");
        self.job =
            Some(Job::spawn(label, done, move || backend.connect(&ssid, password.as_deref())));
    }

    /// Connects (pairing first if new) to a device on a thread, so the screen stays alive.
    fn start_bluetooth(&mut self, device: &Device) {
        if self.busy() {
            return;
        }
        let backend = self.bt.clone();
        let (address, label) = (
            device.address.clone(),
            if device.paired {
                format!("connecting to {}", device.name)
            } else {
                format!("pairing with {}", device.name)
            },
        );
        let done = format!("{label} done");
        self.job = Some(Job::spawn(label, done, move || backend.connect(&address)));
    }

    /// Moves the focus to a pane.
    fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
    }

    /// Moves the focus by `delta` panes, wrapping around.
    fn focus_by(&mut self, delta: isize) {
        let at = ORDER.iter().position(|f| *f == self.focus).unwrap_or(0) as isize;
        self.focus = ORDER[(at + delta).rem_euclid(ORDER.len() as isize) as usize];
    }

    /// Runs an action by id: the focus ones here, the per-pane ones in their own handlers.
    fn run_action_id(&mut self, id: &'static str) -> Request {
        match id {
            "focus.next" => self.focus_by(1),
            "focus.prev" => self.focus_by(-1),
            "focus.wifi" => self.set_focus(Focus::Wifi),
            "focus.bluetooth" => self.set_focus(Focus::Bluetooth),
            "quit" => return Request::Quit,
            _ => match self.focus {
                Focus::Wifi => self.wifi_action(id),
                Focus::Bluetooth => self.bluetooth_action(id),
            },
        }
        Request::None
    }

    /// Runs a Wi-Fi action on the network under the cursor.
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
                    self.show_message("disconnected", false);
                    self.refresh();
                }
                Err(e) => self.show_message(e.to_string(), true),
            },
            "wifi.forget" => match self.network() {
                Some(n) if n.known => self.mode = Mode::ForgetWifi(n.ssid.clone()),
                Some(_) => self.show_message("no saved connection for this network", true),
                None => {}
            },
            "wifi.scan" => match self.wifi.scan() {
                Ok(()) => {
                    self.show_message("scanning (the list updates in a few seconds)", false);
                    self.refresh();
                }
                Err(e) => self.show_message(e.to_string(), true),
            },
            "wifi.power" => {
                let on = !self.wifi_status.enabled;
                match self.wifi.set_enabled(on) {
                    Ok(()) => {
                        self.show_message(if on { "Wi-Fi on" } else { "Wi-Fi off" }, false);
                        self.refresh();
                    }
                    Err(e) => self.show_message(e.to_string(), true),
                }
            }
            _ => {}
        }
    }

    /// Runs a Bluetooth action on the device under the cursor.
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
                        self.show_message(format!("disconnected {}", d.name), false);
                        self.refresh();
                    }
                    Err(e) => self.show_message(e.to_string(), true),
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
                        self.show_message(
                            if d.trusted {
                                format!("{} is no longer trusted", d.name)
                            } else {
                                format!("trusting {}", d.name)
                            },
                            false,
                        );
                        self.refresh();
                    }
                    Err(e) => self.show_message(e.to_string(), true),
                }
            }
            "bt.scan" => {
                let on = !self.bt_status.discovering;
                match self.bt.scan(on) {
                    Ok(()) => {
                        self.show_message(
                            if on { "looking for devices" } else { "stopped looking" },
                            false,
                        );
                        self.refresh();
                    }
                    Err(e) => self.show_message(e.to_string(), true),
                }
            }
            "bt.power" => {
                let on = !self.bt_status.powered;
                match self.bt.set_powered(on) {
                    Ok(()) => {
                        self.show_message(if on { "Bluetooth on" } else { "Bluetooth off" }, false);
                        self.refresh();
                    }
                    Err(e) => self.show_message(e.to_string(), true),
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

/// Lets the shared app shell host the screen: it owns the keymaps, the help and the palette; the
/// screen draws and runs its own actions.
impl ui::app::Screen for App {
    fn render(&mut self, area: Rect, buf: &mut Buffer, _theme: &Theme) {
        App::render(self, area, buf);
    }

    /// A password prompt or a forget question is open: the keys are theirs.
    fn captures_input(&self, _ev: &Event) -> bool {
        !matches!(self.mode, Mode::Normal)
    }

    fn on_event(&mut self, ev: &Event, _host: &mut dyn Host) -> Request {
        self.typing(ev);
        Request::None
    }

    fn run_action(&mut self, id: &'static str, _host: &mut dyn Host) -> Request {
        self.run_action_id(id)
    }

    fn on_keypress(&mut self) {
        self.message = None;
    }

    fn on_unbound_keys(&mut self, keys: &str) {
        self.show_message(format!("{keys} is not bound"), true);
    }

    fn on_tick(&mut self) -> bool {
        App::tick(self)
    }

    fn navigable(&mut self) -> Option<&mut dyn Navigable> {
        Some(self)
    }

    /// The focused pane: Wi-Fi and Bluetooth have different actions.
    fn context(&self) -> usize {
        Self::pane(self.focus)
    }
}

/// The cursor of the focused list is what the shared movement walks.
impl Navigable for App {
    fn move_by(&mut self, delta: isize) {
        let len = self.len(self.focus);
        let st = &mut self.lists[Self::pane(self.focus)];
        st.cursor = ui::list::move_cursor(st.cursor, delta, len);
    }

    fn page_row_count(&self) -> usize {
        10
    }

    fn move_to_top(&mut self) {
        self.lists[Self::pane(self.focus)].cursor = 0;
    }

    fn move_to_bottom(&mut self) {
        let last = self.len(self.focus).saturating_sub(1);
        self.lists[Self::pane(self.focus)].cursor = last;
    }
}
