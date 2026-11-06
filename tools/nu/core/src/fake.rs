//! A pretend radio with a few networks, for tests and for trying the UI without hardware.

use crate::bluetooth::{Bluetooth, BtStatus, Device, Kind};
use crate::{Backend, Error, Network, Result, Security, Status, merge};
use std::sync::Mutex;

pub struct Fake {
    state: Mutex<State>,
}

struct State {
    enabled: bool,
    networks: Vec<Network>,
    /// Calls made, in order (`connect home`, `scan`, ...), for tests to look at.
    log: Vec<String>,
}

impl Fake {
    /// A radio with `home` (protected, saved), `cafe` (open) and `office` (802.1X) in range.
    pub fn new() -> Fake {
        let n = |ssid: &str, strength: u8, security: Security, known: bool| Network { ssid: ssid.into(), strength, security, active: false, known, frequency_mhz: 5180, bssid: format!("aa:bb:cc:00:00:{strength:02x}") };
        Fake {
            state: Mutex::new(State {
                enabled: true,
                networks: vec![n("home", 82, Security::Psk, true), n("cafe", 55, Security::Open, false), n("office", 40, Security::Enterprise, false), n("neighbor", 25, Security::Psk, false)],
                log: Vec::new(),
            }),
        }
    }

    pub fn log(&self) -> Vec<String> {
        self.state.lock().expect("fake state").log.clone()
    }
}

impl Default for Fake {
    fn default() -> Self {
        Fake::new()
    }
}

impl Backend for Fake {
    fn status(&self) -> Result<Status> {
        let s = self.state.lock().expect("fake state");
        Ok(Status { device: Some("wlan0".into()), enabled: s.enabled, active: s.networks.iter().find(|n| n.active).map(|n| n.ssid.clone()) })
    }

    fn networks(&self) -> Result<Vec<Network>> {
        let s = self.state.lock().expect("fake state");
        if !s.enabled {
            return Ok(Vec::new());
        }
        Ok(merge(s.networks.clone()))
    }

    fn scan(&self) -> Result<()> {
        self.state.lock().expect("fake state").log.push("scan".into());
        Ok(())
    }

    fn set_enabled(&self, on: bool) -> Result<()> {
        let mut s = self.state.lock().expect("fake state");
        s.log.push(format!("enabled {on}"));
        s.enabled = on;
        if !on {
            s.networks.iter_mut().for_each(|n| n.active = false);
        }
        Ok(())
    }

    fn connect(&self, ssid: &str, password: Option<&str>) -> Result<()> {
        let mut s = self.state.lock().expect("fake state");
        s.log.push(format!("connect {ssid}"));
        let Some(i) = s.networks.iter().position(|n| n.ssid == ssid) else { return Err(Error(format!("{ssid} is not in range"))) };
        let (security, known) = (s.networks[i].security, s.networks[i].known);
        match security {
            Security::Enterprise => return Err(Error("802.1X networks are not supported yet".into())),
            Security::Open => {}
            _ if known => {}
            _ => match password {
                None => return Err(Error("a password is needed".into())),
                Some(p) if p.len() < 8 => return Err(Error("Secrets were required, but not provided".into())),
                Some(_) => {}
            },
        }
        s.networks.iter_mut().for_each(|n| n.active = false);
        s.networks[i].active = true;
        s.networks[i].known = true;
        Ok(())
    }

    fn disconnect(&self) -> Result<()> {
        let mut s = self.state.lock().expect("fake state");
        s.log.push("disconnect".into());
        s.networks.iter_mut().for_each(|n| n.active = false);
        Ok(())
    }

    fn forget(&self, ssid: &str) -> Result<()> {
        let mut s = self.state.lock().expect("fake state");
        s.log.push(format!("forget {ssid}"));
        match s.networks.iter_mut().find(|n| n.ssid == ssid && n.known) {
            Some(n) => {
                n.known = false;
                n.active = false;
                Ok(())
            }
            None => Err(Error(format!("no saved connection for {ssid}"))),
        }
    }
}

/// A pretend Bluetooth adapter with a few devices.
pub struct FakeBluetooth {
    state: Mutex<BtState>,
}

struct BtState {
    powered: bool,
    discovering: bool,
    devices: Vec<Device>,
    log: Vec<String>,
}

impl FakeBluetooth {
    /// `Headphones` (paired, in range), `Keyboard` (paired, connected), `Phone` and `Speaker`
    /// (in range, not paired) and `Old mouse` (paired, away).
    pub fn new() -> FakeBluetooth {
        let d = |name: &str, kind: Kind, paired: bool, connected: bool, rssi: Option<i16>| Device { address: format!("00:11:22:33:44:{:02X}", name.len()), name: name.into(), kind, paired, trusted: paired, connected, rssi };
        FakeBluetooth {
            state: Mutex::new(BtState {
                powered: true,
                discovering: false,
                devices: vec![d("Headphones", Kind::Audio, true, false, Some(-58)), d("Keyboard", Kind::Keyboard, true, true, Some(-47)), d("Phone", Kind::Phone, false, false, Some(-72)), d("Speaker", Kind::Audio, false, false, Some(-80)), d("Old mouse", Kind::Mouse, true, false, None)],
                log: Vec::new(),
            }),
        }
    }

    pub fn log(&self) -> Vec<String> {
        self.state.lock().expect("fake state").log.clone()
    }
}

impl Default for FakeBluetooth {
    fn default() -> Self {
        FakeBluetooth::new()
    }
}

impl Bluetooth for FakeBluetooth {
    fn status(&self) -> Result<BtStatus> {
        let s = self.state.lock().expect("fake state");
        Ok(BtStatus { adapter: Some("hci0".into()), powered: s.powered, discovering: s.discovering })
    }

    fn devices(&self) -> Result<Vec<Device>> {
        let s = self.state.lock().expect("fake state");
        if !s.powered {
            return Ok(Vec::new());
        }
        let mut v: Vec<Device> = s.devices.iter().filter(|d| d.paired || d.connected || d.rssi.is_some()).cloned().collect();
        crate::bluetooth::sort(&mut v);
        Ok(v)
    }

    fn scan(&self, on: bool) -> Result<()> {
        let mut s = self.state.lock().expect("fake state");
        s.log.push(format!("scan {on}"));
        s.discovering = on;
        Ok(())
    }

    fn set_powered(&self, on: bool) -> Result<()> {
        let mut s = self.state.lock().expect("fake state");
        s.log.push(format!("powered {on}"));
        s.powered = on;
        if !on {
            s.devices.iter_mut().for_each(|d| d.connected = false);
        }
        Ok(())
    }

    fn connect(&self, address: &str) -> Result<()> {
        let mut s = self.state.lock().expect("fake state");
        s.log.push(format!("connect {address}"));
        let d = s.devices.iter_mut().find(|d| d.address == address).ok_or_else(|| Error(format!("{address} is not known")))?;
        if d.rssi.is_none() && !d.connected {
            return Err(Error("the device is not in range".into()));
        }
        d.paired = true;
        d.trusted = true;
        d.connected = true;
        Ok(())
    }

    fn disconnect(&self, address: &str) -> Result<()> {
        let mut s = self.state.lock().expect("fake state");
        s.log.push(format!("disconnect {address}"));
        let d = s.devices.iter_mut().find(|d| d.address == address).ok_or_else(|| Error(format!("{address} is not known")))?;
        d.connected = false;
        Ok(())
    }

    fn set_trusted(&self, address: &str, on: bool) -> Result<()> {
        let mut s = self.state.lock().expect("fake state");
        s.log.push(format!("trusted {address} {on}"));
        let d = s.devices.iter_mut().find(|d| d.address == address).ok_or_else(|| Error(format!("{address} is not known")))?;
        d.trusted = on;
        Ok(())
    }

    fn remove(&self, address: &str) -> Result<()> {
        let mut s = self.state.lock().expect("fake state");
        s.log.push(format!("remove {address}"));
        let before = s.devices.len();
        s.devices.retain(|d| d.address != address);
        if s.devices.len() == before { Err(Error(format!("{address} is not known"))) } else { Ok(()) }
    }
}

#[cfg(test)]
mod tests;
