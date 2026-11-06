use super::*;

#[test]
fn security_comes_from_the_flags() {
    assert_eq!(security_of(0, 0, 0), Security::Open);
    assert_eq!(security_of(AP_FLAGS_PRIVACY, 0, 0), Security::Wep);
    assert_eq!(security_of(AP_FLAGS_PRIVACY, 0x188, 0), Security::Psk);
    assert_eq!(security_of(AP_FLAGS_PRIVACY, 0, 0x188), Security::Psk);
    assert_eq!(security_of(AP_FLAGS_PRIVACY, 0, 0x200), Security::Enterprise);
}

#[test]
fn ssids_are_text_even_when_the_bytes_are_not_valid_utf8() {
    assert_eq!(ssid_text(b"home"), "home");
    assert_eq!(ssid_text("café".as_bytes()), "café");
    assert!(ssid_text(&[0x68, 0xff, 0x69]).contains('\u{fffd}'));
}
