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

#[test]
fn a_scale_label_has_no_needless_decimal() {
    assert_eq!(rate_label(5.0 * 1024.0), "5K/s");
    assert_eq!(rate_label(1024.0 * 1024.0), "1M/s");
    assert_eq!(rate_label(2.5 * 1024.0), "2.5K/s");
    assert_eq!(rate_label(10.0 * 1024.0), "10K/s");
}

#[test]
fn the_steps_of_a_scale_only_grow_and_end_at_ten_gigabytes() {
    assert!(RATE_STEPS.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(rate_label(RATE_STEPS[0]), "5K/s");
    assert_eq!(rate_label(RATE_STEPS[RATE_STEPS.len() - 1]), "10G/s");
}

#[test]
fn a_rate_is_hot_by_the_power_of_ten_it_reaches() {
    assert_eq!(rate_load(0), 0.0);
    assert_eq!(rate_load(100), 0.0, "under 1K/s is cold");
    assert!((rate_load(1_000_000) - 0.5).abs() < 0.01, "1M/s is half way");
    assert_eq!(rate_load(u64::MAX), 1.0);
}
