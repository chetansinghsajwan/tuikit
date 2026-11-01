//! Bluetooth: devices behind one `Bluetooth` trait. The real backend is BlueZ over D-Bus
//! (`bluez`); `fake::FakeBluetooth` serves tests and UIs.

use crate::Result;

/// What a device is, from BlueZ's icon name (`audio-headphones`, `input-keyboard`, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Audio,
    Keyboard,
    Mouse,
    Gamepad,
    Phone,
    Computer,
    Other,
}

impl Kind {
    pub fn from_icon(icon: &str) -> Kind {
        match icon {
            i if i.starts_with("audio") => Kind::Audio,
            "input-keyboard" => Kind::Keyboard,
            "input-mouse" | "input-tablet" => Kind::Mouse,
            "input-gaming" => Kind::Gamepad,
            "phone" | "multimedia-player" => Kind::Phone,
            "computer" => Kind::Computer,
            _ => Kind::Other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Audio => "audio",
            Kind::Keyboard => "keyboard",
            Kind::Mouse => "mouse",
            Kind::Gamepad => "gamepad",
            Kind::Phone => "phone",
            Kind::Computer => "computer",
            Kind::Other => "device",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// `AA:BB:CC:DD:EE:FF`: what identifies it.
    pub address: String,
    /// The name shown (the alias the user may have set, else the device's own name).
    pub name: String,
    pub kind: Kind,
    pub paired: bool,
    pub trusted: bool,
    pub connected: bool,
    /// Signal in dBm while it is in range; `None` when it is not seen right now.
    pub rssi: Option<i16>,
}

/// What the Bluetooth adapter is doing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BtStatus {
    /// The adapter (`hci0`); `None` when the machine has none.
    pub adapter: Option<String>,
    pub powered: bool,
    pub discovering: bool,
}

pub trait Bluetooth: Send + Sync {
    fn status(&self) -> Result<BtStatus>;
    /// Known and visible devices: connected first, then paired, then by signal.
    fn devices(&self) -> Result<Vec<Device>>;
    /// Start or stop looking for devices.
    fn scan(&self, on: bool) -> Result<()>;
    fn set_powered(&self, on: bool) -> Result<()>;
    /// Connect to a device. A device not paired yet is paired and trusted first (it works for
    /// devices that pair without a PIN: headphones, mice, most keyboards).
    fn connect(&self, address: &str) -> Result<()>;
    fn disconnect(&self, address: &str) -> Result<()>;
    fn set_trusted(&self, address: &str, on: bool) -> Result<()>;
    /// Forget the device: unpair it and delete what is known about it.
    fn remove(&self, address: &str) -> Result<()>;
}

/// Order for display: connected, then paired, then by signal, then by name.
pub fn sort(devices: &mut [Device]) {
    devices.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then(b.paired.cmp(&a.paired))
            .then(b.rssi.unwrap_or(i16::MIN).cmp(&a.rssi.unwrap_or(i16::MIN)))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

/// A signal in dBm as 0 to 100, for the same bars as Wi-Fi (about -100 dBm is nothing, -40 is as good as it gets).
pub fn rssi_percent(rssi: i16) -> u8 {
    ((i32::from(rssi) + 100) * 100 / 60).clamp(0, 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, paired: bool, connected: bool, rssi: Option<i16>) -> Device {
        Device { address: name.into(), name: name.into(), kind: Kind::Other, paired, trusted: paired, connected, rssi }
    }

    #[test]
    fn kinds_come_from_icons() {
        assert_eq!(Kind::from_icon("audio-headphones"), Kind::Audio);
        assert_eq!(Kind::from_icon("audio-card"), Kind::Audio);
        assert_eq!(Kind::from_icon("input-keyboard"), Kind::Keyboard);
        assert_eq!(Kind::from_icon("input-mouse"), Kind::Mouse);
        assert_eq!(Kind::from_icon("phone"), Kind::Phone);
        assert_eq!(Kind::from_icon(""), Kind::Other);
    }

    #[test]
    fn sort_puts_connected_then_paired_then_strongest_first() {
        let mut v = vec![dev("far", false, false, Some(-90)), dev("near", false, false, Some(-50)), dev("known", true, false, None), dev("live", true, true, Some(-70)), dev("alpha", false, false, None)];
        sort(&mut v);
        assert_eq!(v.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), vec!["live", "known", "near", "far", "alpha"]);
    }

    #[test]
    fn rssi_maps_to_a_percentage() {
        assert_eq!(rssi_percent(-100), 0);
        assert_eq!(rssi_percent(-70), 50);
        assert_eq!(rssi_percent(-40), 100);
        assert_eq!(rssi_percent(-20), 100);
        assert_eq!(rssi_percent(-120), 0);
    }
}
