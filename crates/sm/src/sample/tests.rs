//! Tests for the history and the ratio.

use super::*;

#[test]
fn ring_drops_the_oldest_reading_when_full() {
    let mut ring = Ring::default();
    for i in 0..HISTORY_SIZE + 5 {
        ring.push(i as f32);
    }
    let values = ring.values();
    assert_eq!(values.len(), HISTORY_SIZE);
    assert_eq!(values[0], 5.0);
}

#[test]
fn scaled_never_divides_by_less_than_the_floor() {
    let mut ring = Ring::default();
    ring.push(50.0);
    ring.push(100.0);
    assert_eq!(ring.scaled(1000.0), [0.05, 0.1]);
    assert_eq!(ring.scaled(10.0), [0.5, 1.0]);
}

#[test]
fn ratio_is_zero_without_a_total() {
    assert_eq!(ratio(5, 0), 0.0);
    assert_eq!(ratio(1, 4), 0.25);
}

#[test]
fn the_machine_can_be_read() {
    let Ok(sampler) = Sampler::new() else { return }; // a sandbox may hide the machine
    assert!(sampler.snapshot.memory_total > 0);
    assert!(!sampler.snapshot.procs.is_empty());
}
