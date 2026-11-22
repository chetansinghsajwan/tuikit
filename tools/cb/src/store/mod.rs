//! The clipboard history on disk: its file format, location and the rules for adding to it.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Entries kept; the oldest go first.
pub const MAX_ENTRIES: usize = 1000;
/// Separates entries in the history file. Text cannot hold a NUL, so it never clashes.
const SEP: char = '\0';

/// One remembered copy.
pub struct Entry {
    /// Seconds since the epoch when it was copied.
    pub time: u64,
    /// The copied text.
    pub text: String,
}

/// The history file, under the user's state directory.
fn history_path() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("cb").join("history")
}

/// Parses the history file contents (`time\ntext` entries separated by NUL), oldest first.
pub fn parse(data: &str) -> Vec<Entry> {
    data.split(SEP)
        .filter_map(|rec| {
            let (time, text) = rec.split_once('\n')?;
            Some(Entry { time: time.parse().ok()?, text: text.to_string() })
        })
        .collect()
}

/// Writes the entries in the file format.
pub fn serialize(entries: &[Entry]) -> String {
    entries.iter().map(|e| format!("{}\n{}{SEP}", e.time, e.text)).collect()
}

/// Reads the history; a missing file is an empty history.
pub fn load() -> Vec<Entry> {
    fs::read_to_string(history_path()).map(|d| parse(&d)).unwrap_or_default()
}

/// Replaces the history file, creating its directory.
pub fn save(entries: &[Entry]) -> io::Result<()> {
    let path = history_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serialize(entries))
}

/// Adds `text` as the newest entry: an older copy of the same text is dropped, so each text
/// appears once, and the oldest entries beyond the cap go. Blank text is not recorded.
pub fn push(entries: &mut Vec<Entry>, text: &str, time: u64) {
    if text.trim().is_empty() {
        return;
    }
    entries.retain(|e| e.text != text);
    entries.push(Entry { time, text: text.to_string() });
    let extra = entries.len().saturating_sub(MAX_ENTRIES);
    entries.drain(..extra);
}

/// The current time in seconds since the epoch.
fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Records `text` as the newest entry of the history file.
pub fn record(text: &str) -> io::Result<()> {
    let mut entries = load();
    push(&mut entries, text, now());
    save(&entries)
}

/// Removes every entry with this text from the history file.
pub fn remove(text: &str) -> io::Result<()> {
    let mut entries = load();
    entries.retain(|e| e.text != text);
    save(&entries)
}

#[cfg(test)]
mod tests;
