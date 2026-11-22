//! Recording: `cb store` records one text, `cb watch` records every copy until stopped.

use crate::backend::Backend;
use crate::store;
use std::io::{self, Read};
use std::time::Duration;

/// How often `watch` reads the clipboard. Polling, because `wl-paste --watch` needs the
/// data-control protocol (WSLg's compositor does not offer it) and macOS has no change event a
/// command-line tool can wait on.
const POLL: Duration = Duration::from_millis(500);

/// Records the text on stdin as the newest entry.
pub fn store() -> io::Result<u8> {
    let mut text = String::new();
    io::stdin().read_to_string(&mut text)?;
    store::record(&text)?;
    Ok(0)
}

/// Reads the clipboard every `POLL` and records each new text, until stopped. A failure to reach
/// the clipboard is printed once per distinct message, so the service log shows it without
/// repeating twice a second; recording resumes as soon as the clipboard is reachable again.
pub fn watch(backend: &dyn Backend) -> io::Result<u8> {
    let mut last = String::new();
    let mut last_error = String::new();
    loop {
        match backend.read() {
            Ok(Some(text)) if text != last => {
                store::record(&text)?;
                last = text;
                last_error.clear();
            }
            Ok(_) => last_error.clear(),
            Err(e) if e != last_error => {
                eprintln!("cb: {e}");
                last_error = e;
            }
            Err(_) => {}
        }
        std::thread::sleep(POLL);
    }
}
