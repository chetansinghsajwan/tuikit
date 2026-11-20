//! A job: work that waits for the network, a device or a remote, run on its own thread so the
//! screen stays alive. The tool keeps one in an `Option<Job<T>>`, shows its label in the footer
//! while it runs, and calls `finished` between events to collect it.

use std::thread::JoinHandle;

/// A running piece of work that ends with a `T`.
pub struct Job<T> {
    label: String,
    done_text: String,
    thread: JoinHandle<T>,
}

/// A job that has ended.
pub struct Done<T> {
    /// What to say when the job succeeded.
    pub done_text: String,
    /// What the work returned; `None` when the thread crashed.
    pub result: Option<T>,
}

impl<T: Send + 'static> Job<T> {
    /// Starts `work` on a thread. `label` is shown while it runs, `done_text` after it succeeds.
    pub fn spawn(
        label: impl Into<String>,
        done_text: impl Into<String>,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Job<T> {
        Job { label: label.into(), done_text: done_text.into(), thread: std::thread::spawn(work) }
    }
}

impl<T> Job<T> {
    /// What the job is doing, for the footer.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Collects the job in `slot` if it has ended, leaving the slot empty; `None` while it still runs
/// or when there is no job.
pub fn finished<T>(slot: &mut Option<Job<T>>) -> Option<Done<T>> {
    if !slot.as_ref().is_some_and(|j| j.thread.is_finished()) {
        return None;
    }
    let job = slot.take()?;
    Some(Done { done_text: job.done_text, result: job.thread.join().ok() })
}

#[cfg(test)]
mod tests;
