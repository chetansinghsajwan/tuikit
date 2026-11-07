//! The NetworkManager backend against a pretend NetworkManager on a private D-Bus session bus.
//! It checks that our calls match the methods, signatures and properties of NetworkManager's
//! D-Bus API as documented; it cannot check real NetworkManager. Skipped without `dbus-daemon`.

use nu_core::nm::Nm;
use nu_core::{Backend, Security};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use zbus::blocking::connection::Builder;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

const NAME: &str = "org.freedesktop.NetworkManager";
const DEVICE: &str = "/org/freedesktop/NetworkManager/Devices/1";
const AP_HOME: &str = "/org/freedesktop/NetworkManager/AccessPoint/1";
const AP_HOME_WEAK: &str = "/org/freedesktop/NetworkManager/AccessPoint/2";
const AP_CAFE: &str = "/org/freedesktop/NetworkManager/AccessPoint/3";
const AP_OFFICE: &str = "/org/freedesktop/NetworkManager/AccessPoint/4";
const SAVED_HOME: &str = "/org/freedesktop/NetworkManager/Settings/1";

#[derive(Default)]
struct State {
    enabled: bool,
    active_ap: String,
    home_saved: bool,
    calls: Vec<String>,
}

type Shared = Arc<Mutex<State>>;

fn path(p: &str) -> OwnedObjectPath {
    OwnedObjectPath::try_from(p).expect("a valid object path")
}

fn text(v: &OwnedValue) -> String {
    String::try_from(v.clone()).unwrap_or_default()
}

struct Root(Shared);

#[zbus::interface(name = "org.freedesktop.NetworkManager")]
impl Root {
    fn get_devices(&self) -> Vec<OwnedObjectPath> {
        vec![path("/org/freedesktop/NetworkManager/Devices/0"), path(DEVICE)]
    }

    fn activate_connection(
        &self,
        connection: OwnedObjectPath,
        _device: OwnedObjectPath,
        ap: OwnedObjectPath,
    ) -> OwnedObjectPath {
        let mut s = self.0.lock().unwrap();
        s.calls.push(format!("activate {} {}", connection.as_str(), ap.as_str()));
        s.active_ap = ap.as_str().to_string();
        path("/org/freedesktop/NetworkManager/ActiveConnection/1")
    }

    fn add_and_activate_connection(
        &self,
        settings: HashMap<String, HashMap<String, OwnedValue>>,
        _device: OwnedObjectPath,
        ap: OwnedObjectPath,
    ) -> (OwnedObjectPath, OwnedObjectPath) {
        let mut s = self.0.lock().unwrap();
        let ssid = settings
            .get("802-11-wireless")
            .and_then(|w| w.get("ssid"))
            .and_then(|v| Vec::<u8>::try_from(v.clone()).ok())
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        let security = settings.get("802-11-wireless-security");
        let key_mgmt = security.and_then(|x| x.get("key-mgmt")).map(text).unwrap_or_else(|| "none-set".into());
        let psk = security.and_then(|x| x.get("psk")).map(text).unwrap_or_default();
        let kind = settings.get("connection").and_then(|c| c.get("type")).map(text).unwrap_or_default();
        s.calls.push(format!("add {ssid} type={kind} key-mgmt={key_mgmt} psk={psk} ap={}", ap.as_str()));
        s.active_ap = ap.as_str().to_string();
        (path("/org/freedesktop/NetworkManager/Settings/9"), path("/org/freedesktop/NetworkManager/ActiveConnection/2"))
    }

    #[zbus(property)]
    fn wireless_enabled(&self) -> bool {
        self.0.lock().unwrap().enabled
    }

    #[zbus(property)]
    fn set_wireless_enabled(&mut self, on: bool) {
        let mut s = self.0.lock().unwrap();
        s.calls.push(format!("enabled {on}"));
        s.enabled = on;
    }
}

struct Wired;

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device")]
impl Wired {
    #[zbus(property)]
    fn device_type(&self) -> u32 {
        1
    }

    #[zbus(property, name = "Interface")]
    fn iface(&self) -> String {
        "eth0".into()
    }
}

struct Wifi(Shared);

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device")]
impl Wifi {
    #[zbus(property)]
    fn device_type(&self) -> u32 {
        2
    }

    #[zbus(property, name = "Interface")]
    fn iface(&self) -> String {
        "wlan0".into()
    }

    fn disconnect(&self) {
        let mut s = self.0.lock().unwrap();
        s.calls.push("disconnect".into());
        s.active_ap = "/".into();
    }
}

struct Wireless(Shared);

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device.Wireless")]
impl Wireless {
    fn get_access_points(&self) -> Vec<OwnedObjectPath> {
        [AP_HOME, AP_HOME_WEAK, AP_CAFE, AP_OFFICE].iter().map(|p| path(p)).collect()
    }

    fn request_scan(&self, _options: HashMap<String, OwnedValue>) {
        self.0.lock().unwrap().calls.push("scan".into());
    }

    #[zbus(property)]
    fn active_access_point(&self) -> OwnedObjectPath {
        path(&self.0.lock().unwrap().active_ap)
    }
}

struct Ap {
    ssid: &'static str,
    strength: u8,
    flags: u32,
    wpa: u32,
    rsn: u32,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl Ap {
    #[zbus(property)]
    fn ssid(&self) -> Vec<u8> {
        self.ssid.as_bytes().to_vec()
    }
    #[zbus(property)]
    fn strength(&self) -> u8 {
        self.strength
    }
    #[zbus(property)]
    fn flags(&self) -> u32 {
        self.flags
    }
    #[zbus(property)]
    fn wpa_flags(&self) -> u32 {
        self.wpa
    }
    #[zbus(property)]
    fn rsn_flags(&self) -> u32 {
        self.rsn
    }
    #[zbus(property)]
    fn frequency(&self) -> u32 {
        5180
    }
    #[zbus(property)]
    fn hw_address(&self) -> String {
        "aa:bb:cc:dd:ee:ff".into()
    }
}

struct Settings(Shared);

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings")]
impl Settings {
    fn list_connections(&self) -> Vec<OwnedObjectPath> {
        if self.0.lock().unwrap().home_saved { vec![path(SAVED_HOME)] } else { Vec::new() }
    }
}

struct Saved(Shared);

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings.Connection")]
impl Saved {
    fn get_settings(&self) -> HashMap<String, HashMap<String, OwnedValue>> {
        let v = |x: Value<'_>| OwnedValue::try_from(x).unwrap();
        HashMap::from([
            (
                "connection".to_string(),
                HashMap::from([
                    ("type".to_string(), v(Value::from("802-11-wireless"))),
                    ("id".to_string(), v(Value::from("home"))),
                ]),
            ),
            ("802-11-wireless".to_string(), HashMap::from([("ssid".to_string(), v(Value::from(b"home".to_vec())))])),
        ])
    }

    fn delete(&self) {
        let mut s = self.0.lock().unwrap();
        s.calls.push("delete saved home".into());
        s.home_saved = false;
    }
}

struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A private session bus, or `None` when there is no `dbus-daemon` to run.
fn bus() -> Option<(Daemon, String)> {
    let mut child = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--print-address=1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut address = String::new();
    BufReader::new(child.stdout.take()?).read_line(&mut address).ok()?;
    Some((Daemon(child), address.trim().to_string()))
}

fn serve(address: &str, state: &Shared) -> zbus::blocking::Connection {
    let ap = |ssid, strength, flags, wpa, rsn| Ap { ssid, strength, flags, wpa, rsn };
    Builder::address(address)
        .unwrap()
        .name(NAME)
        .unwrap()
        .serve_at("/org/freedesktop/NetworkManager", Root(state.clone()))
        .unwrap()
        .serve_at("/org/freedesktop/NetworkManager/Devices/0", Wired)
        .unwrap()
        .serve_at(DEVICE, Wifi(state.clone()))
        .unwrap()
        .serve_at(DEVICE, Wireless(state.clone()))
        .unwrap()
        .serve_at(AP_HOME, ap("home", 80, 1, 0x188, 0x188))
        .unwrap()
        .serve_at(AP_HOME_WEAK, ap("home", 30, 1, 0x188, 0x188))
        .unwrap()
        .serve_at(AP_CAFE, ap("cafe", 55, 0, 0, 0))
        .unwrap()
        .serve_at(AP_OFFICE, ap("office", 40, 1, 0x200, 0x200))
        .unwrap()
        .serve_at("/org/freedesktop/NetworkManager/Settings", Settings(state.clone()))
        .unwrap()
        .serve_at(SAVED_HOME, Saved(state.clone()))
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn the_backend_speaks_networkmanagers_dbus_api() {
    let Some((_daemon, address)) = bus() else {
        eprintln!("no dbus-daemon: skipped");
        return;
    };
    let state: Shared =
        Arc::new(Mutex::new(State { enabled: true, active_ap: "/".into(), home_saved: true, calls: Vec::new() }));
    let _server = serve(&address, &state);
    let client = Builder::address(address.as_str()).unwrap().build().unwrap();
    let nm = Nm::with_connection(client, NAME);

    // Status and networks: the Wi-Fi device is found among the devices, access points merge per SSID.
    let status = nm.status().unwrap();
    assert_eq!((status.device.as_deref(), status.enabled, status.active.as_deref()), (Some("wlan0"), true, None));
    let nets = nm.networks().unwrap();
    let summary: Vec<(&str, u8, Security, bool, bool)> =
        nets.iter().map(|n| (n.ssid.as_str(), n.strength, n.security, n.known, n.active)).collect();
    assert_eq!(
        summary,
        vec![
            ("home", 80, Security::Psk, true, false),
            ("cafe", 55, Security::Open, false, false),
            ("office", 40, Security::Enterprise, false, false)
        ]
    );

    // A saved network is activated with its saved connection and the strongest access point.
    nm.connect("home", None).unwrap();
    assert!(
        state.lock().unwrap().calls.last().unwrap().ends_with(&format!("activate {SAVED_HOME} {AP_HOME}")),
        "{:?}",
        state.lock().unwrap().calls
    );
    let status = nm.status().unwrap();
    assert_eq!(status.active.as_deref(), Some("home"));
    assert!(nm.networks().unwrap()[0].active);

    // A new open network is added and activated without a security section; a protected one needs a password.
    nm.connect("cafe", None).unwrap();
    assert_eq!(
        state.lock().unwrap().calls.last().unwrap(),
        &format!("add cafe type=802-11-wireless key-mgmt=none-set psk= ap={AP_CAFE}")
    );
    nm.disconnect().unwrap();
    assert_eq!(nm.status().unwrap().active, None);
    assert!(nm.connect("office", Some("x")).unwrap_err().0.contains("802.1X"));
    state.lock().unwrap().home_saved = false;
    assert_eq!(nm.connect("home", None).unwrap_err().0, "a password is needed");
    nm.connect("home", Some("hunter2hunter2")).unwrap();
    assert_eq!(
        state.lock().unwrap().calls.last().unwrap(),
        &format!("add home type=802-11-wireless key-mgmt=wpa-psk psk=hunter2hunter2 ap={AP_HOME}")
    );
    assert!(nm.connect("nowhere", None).unwrap_err().0.contains("not in range"));

    // Scan, power and forget.
    nm.scan().unwrap();
    nm.set_enabled(false).unwrap();
    assert!(!nm.status().unwrap().enabled);
    nm.set_enabled(true).unwrap();
    state.lock().unwrap().home_saved = true;
    nm.forget("home").unwrap();
    assert!(!state.lock().unwrap().home_saved);
    assert!(nm.forget("home").unwrap_err().0.contains("no saved connection"));
    let calls = state.lock().unwrap().calls.clone();
    for want in ["scan", "enabled false", "enabled true", "delete saved home", "disconnect"] {
        assert!(calls.iter().any(|c| c == want), "{want} in {calls:?}");
    }
    let _ = ObjectPath::try_from("/").unwrap();
}
