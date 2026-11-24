//! Tests for the formats.

use super::*;

#[test]
fn bytes_use_binary_units_and_one_decimal_below_ten() {
    assert_eq!(bytes(0), "0B");
    assert_eq!(bytes(1023), "1023B");
    assert_eq!(bytes(1536), "1.5K");
    assert_eq!(bytes(12 * 1024 * 1024), "12M");
}

#[test]
fn rate_adds_per_second() {
    assert_eq!(rate(2048), "2.0K/s");
}

#[test]
fn duration_shows_the_two_largest_units() {
    assert_eq!(duration(65), "1m 5s");
    assert_eq!(duration(3 * 3600 + 4 * 60 + 9), "3h 4m");
    assert_eq!(duration(2 * 86_400 + 5 * 3600), "2d 5h");
}
