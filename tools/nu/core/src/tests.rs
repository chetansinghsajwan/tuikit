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
