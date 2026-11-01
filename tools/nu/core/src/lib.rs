//! `nu-core`: connections (Wi-Fi first) behind one `Backend` trait, with no UI. The real
//! backend talks to NetworkManager over D-Bus (`nm`); a fake one (`fake`) serves tests and
//! UIs. Other kinds of link (Bluetooth, wired, VPN) get their own backends later.

pub mod bluetooth;
pub mod bluez;
pub mod fake;
pub mod nm;

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<zbus::Error> for Error {
    fn from(e: zbus::Error) -> Self {
        // D-Bus errors read `org.freedesktop.NetworkManager.X: message`; the message is what helps.
        let text = e.to_string();
        Error(text.rsplit(": ").next().unwrap_or(&text).to_string())
    }
}

impl From<zbus::fdo::Error> for Error {
    fn from(e: zbus::fdo::Error) -> Self {
        Error::from(zbus::Error::from(e))
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// How a network is protected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    Open,
    Wep,
    /// A shared password (WPA/WPA2/WPA3 personal).
    Psk,
    /// 802.1X (a username and a certificate or a password): not set up by `nu`.
    Enterprise,
}

impl Security {
    pub fn is_open(self) -> bool {
        self == Security::Open
    }

    pub fn label(self) -> &'static str {
        match self {
            Security::Open => "open",
            Security::Wep => "WEP",
            Security::Psk => "WPA",
            Security::Enterprise => "802.1X",
        }
    }
}

/// A Wi-Fi network as seen from here: the strongest access point of an SSID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    pub ssid: String,
    /// 0 to 100.
    pub strength: u8,
    pub security: Security,
    /// Connected right now.
    pub active: bool,
    /// A saved connection exists for it.
    pub known: bool,
    pub frequency_mhz: u32,
    pub bssid: String,
}

/// What the Wi-Fi radio is doing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Status {
    /// The Wi-Fi interface (`wlan0`); `None` when the machine has none.
    pub device: Option<String>,
    pub enabled: bool,
    /// The SSID connected to, if any.
    pub active: Option<String>,
}

/// A source of connections. Methods block for a D-Bus round trip or so; a UI runs the ones
/// that wait for a network (`connect`) on a thread.
pub trait Backend: Send + Sync {
    fn status(&self) -> Result<Status>;
    /// The networks in range, strongest first, the connected one on top.
    fn networks(&self) -> Result<Vec<Network>>;
    /// Ask the radio for a fresh scan; the result shows up in `networks` a few seconds later.
    fn scan(&self) -> Result<()>;
    fn set_enabled(&self, on: bool) -> Result<()>;
    /// Connect to `ssid`: with a saved connection if there is one, else with `password`
    /// (needed for protected networks) which is then saved.
    fn connect(&self, ssid: &str, password: Option<&str>) -> Result<()>;
    fn disconnect(&self) -> Result<()>;
    /// Delete the saved connection(s) of `ssid`.
    fn forget(&self, ssid: &str) -> Result<()>;
}

/// A backend that could not start (no D-Bus, no NetworkManager, no BlueZ): every call reports
/// why, so a UI can still show its panes with the reason.
pub struct Unavailable(pub String);

impl Backend for Unavailable {
    fn status(&self) -> Result<Status> {
        Err(Error(self.0.clone()))
    }
    fn networks(&self) -> Result<Vec<Network>> {
        Err(Error(self.0.clone()))
    }
    fn scan(&self) -> Result<()> {
        Err(Error(self.0.clone()))
    }
    fn set_enabled(&self, _: bool) -> Result<()> {
        Err(Error(self.0.clone()))
    }
    fn connect(&self, _: &str, _: Option<&str>) -> Result<()> {
        Err(Error(self.0.clone()))
    }
    fn disconnect(&self) -> Result<()> {
        Err(Error(self.0.clone()))
    }
    fn forget(&self, _: &str) -> Result<()> {
        Err(Error(self.0.clone()))
    }
}

impl bluetooth::Bluetooth for Unavailable {
    fn status(&self) -> Result<bluetooth::BtStatus> {
        Err(Error(self.0.clone()))
    }
    fn devices(&self) -> Result<Vec<bluetooth::Device>> {
        Err(Error(self.0.clone()))
    }
    fn scan(&self, _: bool) -> Result<()> {
        Err(Error(self.0.clone()))
    }
    fn set_powered(&self, _: bool) -> Result<()> {
        Err(Error(self.0.clone()))
    }
    fn connect(&self, _: &str) -> Result<()> {
        Err(Error(self.0.clone()))
    }
    fn disconnect(&self, _: &str) -> Result<()> {
        Err(Error(self.0.clone()))
    }
    fn set_trusted(&self, _: &str, _: bool) -> Result<()> {
        Err(Error(self.0.clone()))
    }
    fn remove(&self, _: &str) -> Result<()> {
        Err(Error(self.0.clone()))
    }
}

/// Order for display: the connected network first, then by strength, then by name.
pub fn sort(networks: &mut [Network]) {
    networks.sort_by(|a, b| b.active.cmp(&a.active).then(b.strength.cmp(&a.strength)).then_with(|| a.ssid.to_lowercase().cmp(&b.ssid.to_lowercase())));
}

/// The strongest access point per SSID (hidden networks, with an empty SSID, are dropped).
pub fn merge(aps: Vec<Network>) -> Vec<Network> {
    let mut out: Vec<Network> = Vec::new();
    for ap in aps.into_iter().filter(|a| !a.ssid.is_empty()) {
        match out.iter_mut().find(|n| n.ssid == ap.ssid) {
            Some(n) => {
                let active = n.active || ap.active;
                if ap.strength > n.strength {
                    *n = ap;
                }
                n.active = active;
            }
            None => out.push(ap),
        }
    }
    sort(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(ssid: &str, strength: u8, active: bool) -> Network {
        Network { ssid: ssid.into(), strength, security: Security::Psk, active, known: false, frequency_mhz: 2412, bssid: String::new() }
    }

    #[test]
    fn merge_keeps_the_strongest_access_point_and_the_active_flag() {
        let merged = merge(vec![net("home", 40, true), net("home", 80, false), net("cafe", 60, false), net("", 99, false)]);
        let names: Vec<(&str, u8, bool)> = merged.iter().map(|n| (n.ssid.as_str(), n.strength, n.active)).collect();
        // Hidden networks are dropped; the connected one leads even though cafe is not weaker than it.
        assert_eq!(names, vec![("home", 80, true), ("cafe", 60, false)]);
    }

    #[test]
    fn sort_orders_active_then_strength_then_name() {
        let mut v = vec![net("b", 50, false), net("a", 50, false), net("z", 90, false), net("w", 10, true)];
        sort(&mut v);
        assert_eq!(v.iter().map(|n| n.ssid.as_str()).collect::<Vec<_>>(), vec!["w", "z", "a", "b"]);
    }

    #[test]
    fn security_labels_and_openness() {
        assert!(Security::Open.is_open() && !Security::Psk.is_open());
        assert_eq!(Security::Enterprise.label(), "802.1X");
    }
}
