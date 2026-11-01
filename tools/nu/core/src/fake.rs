//! A pretend radio with a few networks, for tests and for trying the UI without hardware.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_needs_a_password_for_new_protected_networks_and_saves_it() {
        let f = Fake::new();
        assert_eq!(f.connect("neighbor", None), Err(Error("a password is needed".into())));
        assert!(f.connect("neighbor", Some("short")).is_err());
        f.connect("neighbor", Some("longenough")).unwrap();
        assert_eq!(f.status().unwrap().active.as_deref(), Some("neighbor"));
        assert!(f.networks().unwrap()[0].known);
        // A saved or open network connects without one, and the previous one is dropped.
        f.connect("cafe", None).unwrap();
        assert_eq!(f.networks().unwrap().iter().filter(|n| n.active).count(), 1);
        assert!(f.connect("office", None).is_err());
        assert!(f.connect("nowhere", None).is_err());
    }

    #[test]
    fn disconnect_forget_and_power() {
        let f = Fake::new();
        f.connect("home", None).unwrap();
        f.disconnect().unwrap();
        assert_eq!(f.status().unwrap().active, None);
        f.forget("home").unwrap();
        assert!(f.forget("home").is_err());
        f.set_enabled(false).unwrap();
        assert!(f.networks().unwrap().is_empty() && !f.status().unwrap().enabled);
        assert_eq!(f.log().last().map(String::as_str), Some("enabled false"));
    }
}
