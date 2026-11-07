//! The BlueZ backend against a pretend BlueZ on a private D-Bus session bus: an adapter and
//! devices under `/org/bluez`, listed through the ObjectManager at `/`. It checks that our
//! calls match BlueZ's D-Bus API as documented; it cannot check real BlueZ or real devices.
//! Skipped without `dbus-daemon`.

use nu_core::bluetooth::{Bluetooth, Kind};
use nu_core::bluez::BlueZ;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use zbus::blocking::connection::Builder;
use zbus::zvariant::OwnedObjectPath;

const NAME: &str = "org.bluez";
const ADAPTER: &str = "/org/bluez/hci0";

#[derive(Clone)]
struct Dev {
    address: &'static str,
    name: &'static str,
    icon: &'static str,
    paired: bool,
    trusted: bool,
    connected: bool,
    rssi: i16,
}

#[derive(Default)]
struct State {
    powered: bool,
    discovering: bool,
    devices: Vec<Dev>,
    calls: Vec<String>,
    removed: Vec<String>,
}

type Shared = Arc<Mutex<State>>;

struct Adapter(Shared);

#[zbus::interface(name = "org.bluez.Adapter1")]
impl Adapter {
    fn start_discovery(&self) {
        let mut s = self.0.lock().unwrap();
        s.calls.push("StartDiscovery".into());
        s.discovering = true;
    }

    fn stop_discovery(&self) {
        let mut s = self.0.lock().unwrap();
        s.calls.push("StopDiscovery".into());
        s.discovering = false;
    }

    fn remove_device(&self, device: OwnedObjectPath) {
        let mut s = self.0.lock().unwrap();
        let tail = device.as_str().rsplit('/').next().unwrap_or("").to_string();
        s.calls.push(format!("RemoveDevice {tail}"));
        // The mock cannot unserve the object, so the removal is recorded rather than applied.
        s.removed.push(tail);
    }

    #[zbus(property)]
    fn powered(&self) -> bool {
        self.0.lock().unwrap().powered
    }

    #[zbus(property)]
    fn set_powered(&mut self, on: bool) {
        let mut s = self.0.lock().unwrap();
        s.calls.push(format!("Powered {on}"));
        s.powered = on;
    }

    #[zbus(property)]
    fn discovering(&self) -> bool {
        self.0.lock().unwrap().discovering
    }
}

struct Device(Shared, usize);

impl Device {
    fn with<T>(&self, f: impl FnOnce(&mut Dev) -> T) -> T {
        f(&mut self.0.lock().unwrap().devices[self.1])
    }
}

#[zbus::interface(name = "org.bluez.Device1")]
impl Device {
    fn pair(&self) {
        let mut s = self.0.lock().unwrap();
        let a = s.devices[self.1].address;
        s.calls.push(format!("Pair {a}"));
        s.devices[self.1].paired = true;
    }

    fn connect(&self) {
        let mut s = self.0.lock().unwrap();
        let a = s.devices[self.1].address;
        s.calls.push(format!("Connect {a}"));
        s.devices[self.1].connected = true;
    }

    fn disconnect(&self) {
        let mut s = self.0.lock().unwrap();
        let a = s.devices[self.1].address;
        s.calls.push(format!("Disconnect {a}"));
        s.devices[self.1].connected = false;
    }

    #[zbus(property)]
    fn address(&self) -> String {
        self.with(|d| d.address.to_string())
    }

    #[zbus(property)]
    fn name(&self) -> String {
        self.with(|d| d.name.to_string())
    }

    #[zbus(property)]
    fn alias(&self) -> String {
        self.with(|d| d.name.to_string())
    }

    #[zbus(property)]
    fn icon(&self) -> String {
        self.with(|d| d.icon.to_string())
    }

    #[zbus(property)]
    fn paired(&self) -> bool {
        self.with(|d| d.paired)
    }

    #[zbus(property)]
    fn trusted(&self) -> bool {
        self.with(|d| d.trusted)
    }

    #[zbus(property)]
    fn set_trusted(&mut self, on: bool) {
        let mut s = self.0.lock().unwrap();
        let a = s.devices[self.1].address;
        s.calls.push(format!("Trusted {a} {on}"));
        s.devices[self.1].trusted = on;
    }

    #[zbus(property)]
    fn connected(&self) -> bool {
        self.with(|d| d.connected)
    }

    #[zbus(property, name = "RSSI")]
    fn rssi(&self) -> i16 {
        self.with(|d| d.rssi)
    }
}

struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

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

#[test]
fn the_backend_speaks_bluezs_dbus_api() {
    let Some((_daemon, address)) = bus() else {
        eprintln!("no dbus-daemon: skipped");
        return;
    };
    let dev = |address, name, icon, paired, connected, rssi| Dev {
        address,
        name,
        icon,
        paired,
        trusted: paired,
        connected,
        rssi,
    };
    let devices = vec![
        dev("AA:00:00:00:00:01", "Headphones", "audio-headphones", true, false, -58),
        dev("AA:00:00:00:00:02", "Keyboard", "input-keyboard", true, true, -47),
        dev("AA:00:00:00:00:03", "Phone", "phone", false, false, -72),
    ];
    let state: Shared = Arc::new(Mutex::new(State {
        powered: true,
        discovering: false,
        devices: devices.clone(),
        calls: Vec::new(),
        removed: Vec::new(),
    }));
    let mut builder = Builder::address(address.as_str())
        .unwrap()
        .name(NAME)
        .unwrap()
        .serve_at("/", zbus::fdo::ObjectManager)
        .unwrap()
        .serve_at(ADAPTER, Adapter(state.clone()))
        .unwrap();
    for (i, d) in devices.iter().enumerate() {
        builder = builder
            .serve_at(
                format!("{ADAPTER}/dev_{}", d.address.replace(':', "_")),
                Device(state.clone(), i),
            )
            .unwrap();
    }
    let _server = builder.build().unwrap();
    let client = Builder::address(address.as_str()).unwrap().build().unwrap();
    let bt = BlueZ::with_connection(client, NAME);

    // Status and devices: connected first, then paired, then by signal.
    let status = bt.status().unwrap();
    assert_eq!(
        (status.adapter.as_deref(), status.powered, status.discovering),
        (Some("hci0"), true, false)
    );
    let list = bt.devices().unwrap();
    let names: Vec<(&str, Kind, bool, bool)> =
        list.iter().map(|d| (d.name.as_str(), d.kind, d.paired, d.connected)).collect();
    assert_eq!(
        names,
        vec![
            ("Keyboard", Kind::Keyboard, true, true),
            ("Headphones", Kind::Audio, true, false),
            ("Phone", Kind::Phone, false, false)
        ]
    );

    // A paired device connects directly; an unpaired one is paired and trusted first.
    bt.connect("AA:00:00:00:00:01").unwrap();
    assert_eq!(state.lock().unwrap().calls.last().unwrap(), "Connect AA:00:00:00:00:01");
    assert!(!state.lock().unwrap().calls.iter().any(|c| c.starts_with("Pair")));
    bt.connect("AA:00:00:00:00:03").unwrap();
    let calls = state.lock().unwrap().calls.clone();
    let tail: Vec<&str> = calls.iter().rev().take(3).rev().map(String::as_str).collect();
    assert_eq!(
        tail,
        vec![
            "Pair AA:00:00:00:00:03",
            "Trusted AA:00:00:00:00:03 true",
            "Connect AA:00:00:00:00:03"
        ]
    );
    assert!(bt.devices().unwrap().iter().all(|d| d.paired && d.trusted));

    // Disconnect, trust, scan, power and remove (forget).
    bt.disconnect("aa:00:00:00:00:02").unwrap(); // addresses match in any case
    assert!(!bt.devices().unwrap().iter().any(|d| d.name == "Keyboard" && d.connected));
    bt.set_trusted("AA:00:00:00:00:01", false).unwrap();
    assert!(bt.devices().unwrap().iter().any(|d| d.name == "Headphones" && !d.trusted));
    bt.scan(true).unwrap();
    assert!(bt.status().unwrap().discovering);
    bt.scan(false).unwrap();
    bt.set_powered(false).unwrap();
    assert!(!bt.status().unwrap().powered);
    bt.set_powered(true).unwrap();
    bt.remove("AA:00:00:00:00:03").unwrap();
    assert_eq!(state.lock().unwrap().calls.last().unwrap(), "RemoveDevice dev_AA_00_00_00_00_03");
    assert_eq!(state.lock().unwrap().removed, vec!["dev_AA_00_00_00_00_03".to_string()]);
    assert!(bt.connect("FF:FF:FF:FF:FF:FF").unwrap_err().0.contains("not known"));
}
