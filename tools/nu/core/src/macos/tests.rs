use super::*;

#[test]
fn channels_map_to_frequencies() {
    assert_eq!((channel_mhz(1), channel_mhz(6), channel_mhz(14), channel_mhz(36), channel_mhz(149)), (2412, 2437, 2484, 5180, 5745));
}

#[test]
fn addresses_compare_without_regard_to_case_or_separator() {
    assert_eq!(normalize("AA-BB-CC-00-11-22"), normalize("aa:bb:cc:00:11:22"));
}

#[test]
fn device_kinds_come_from_the_class_of_device() {
    assert_eq!(kind_of(0x04, 0x06), Kind::Audio);
    assert_eq!(kind_of(0x05, 0x10), Kind::Keyboard);
    assert_eq!(kind_of(0x05, 0x20), Kind::Mouse);
    assert_eq!(kind_of(0x02, 0x03), Kind::Phone);
    assert_eq!(kind_of(0x01, 0x03), Kind::Computer);
    assert_eq!(kind_of(0x1f, 0), Kind::Other);
}
