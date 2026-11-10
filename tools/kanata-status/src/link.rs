//! The connection to `kb daemon`: one JSON object per line, sent on connect and on every change.

use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct State {
    /// Active kanata layer; empty while kanata is unreachable.
    pub layer: String,
    /// Held modifiers: `ctrl`, `shift`, `alt`, `meta`.
    pub mods: Vec<String>,
    /// Held keys by kanata name, in press order.
    pub keys: Vec<String>,
    /// Whether the daemon is connected to kanata.
    pub kanata: bool,
}

pub enum Event {
    State(State),
    /// The daemon went away; the link keeps retrying.
    Down,
}

/// Follows the daemon at `addr` on a thread, reconnecting forever; `send` returns false once
/// nobody listens any more.
pub fn follow(addr: String, send: impl Fn(Event) -> bool + Send + 'static) {
    thread::spawn(move || {
        loop {
            if let Ok(stream) = TcpStream::connect(&addr) {
                let _ = stream.set_nodelay(true);
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    if let Ok(state) = serde_json::from_str(&line)
                        && !send(Event::State(state))
                    {
                        return;
                    }
                }
            }
            if !send(Event::Down) {
                return;
            }
            thread::sleep(Duration::from_secs(2));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::State;

    #[test]
    fn reads_a_daemon_line_with_or_without_keys() {
        let s: State = serde_json::from_str(
            r#"{"layer":"nav","mods":["ctrl"],"keys":["lctl","a"],"kanata":true}"#,
        )
        .unwrap();
        assert_eq!(s.keys, ["lctl", "a"]);
        let old: State =
            serde_json::from_str(r#"{"layer":"main","mods":[],"kanata":true}"#).unwrap();
        assert!(old.keys.is_empty());
    }
}
