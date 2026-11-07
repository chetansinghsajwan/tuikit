use super::*;

fn dev(name: &str, paired: bool, connected: bool, rssi: Option<i16>) -> Device {
    Device {
        address: name.into(),
        name: name.into(),
        kind: Kind::Other,
        paired,
        trusted: paired,
        connected,
        rssi,
    }
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
    let mut v = vec![
        dev("far", false, false, Some(-90)),
        dev("near", false, false, Some(-50)),
        dev("known", true, false, None),
        dev("live", true, true, Some(-70)),
        dev("alpha", false, false, None),
    ];
    sort(&mut v);
    assert_eq!(
        v.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
        vec!["live", "known", "near", "far", "alpha"]
    );
}

#[test]
fn rssi_maps_to_a_percentage() {
    assert_eq!(rssi_percent(-100), 0);
    assert_eq!(rssi_percent(-70), 50);
    assert_eq!(rssi_percent(-40), 100);
    assert_eq!(rssi_percent(-20), 100);
    assert_eq!(rssi_percent(-120), 0);
}
