//! Tests for jobs.

use super::*;
use std::time::{Duration, Instant};

/// Waits until the job in `slot` ends.
fn wait<T>(slot: &mut Option<Job<T>>) -> Done<T> {
    let start = Instant::now();
    loop {
        if let Some(done) = finished(slot) {
            return done;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "the job never ended");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_job_hands_back_its_result_and_empties_the_slot() {
    let mut slot = Some(Job::spawn("working", "worked", || 7));
    assert_eq!(slot.as_ref().map(Job::label), Some("working"));
    let done = wait(&mut slot);
    assert_eq!((done.result, done.done_text.as_str()), (Some(7), "worked"));
    assert!(slot.is_none());
}

/// The job's thread panics on purpose. In the macOS CI build a panic cannot unwind and aborts the
/// test process ("failed to initiate panic, error 5"), so the test runs everywhere else.
#[test]
#[cfg_attr(target_os = "macos", ignore = "a panic aborts the process in the macOS CI build")]
fn a_crashed_job_has_no_result() {
    let mut slot: Option<Job<()>> = Some(Job::spawn("x", "y", || panic!("boom")));
    assert!(wait(&mut slot).result.is_none());
}

#[test]
fn an_empty_slot_has_nothing_to_collect() {
    let mut slot: Option<Job<()>> = None;
    assert!(finished(&mut slot).is_none());
}
