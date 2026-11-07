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
    for want in [
        "Wi-Fi",
        "Bluetooth",
        "home",
        "cafe",
        "saved",
        "WPA",
        "Headphones",
        "Keyboard",
        "audio",
        "paired",
        "Signal",
        "Security",
    ] {
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
    let s = app_with(
        Arc::new(nu_core::Unavailable("NetworkManager is not running".into())),
        Arc::new(FakeBluetooth::new()),
    );
    assert!(
        s.contains("unavailable: NetworkManager is not") && s.contains("running") && s.contains("Headphones"),
        "{s}"
    );
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
