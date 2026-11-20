//! The `cb` binary: a clipboard history. `cb watch` records every text copy, `cb` browses the
//! history and puts the chosen entry back on the clipboard. It is its own tool, with its own
//! store, so it works wherever `wl-clipboard` does (Hyprland and WSLg alike) and does not depend
//! on a desktop session starting a recorder.

use clap::{Parser, Subcommand};
use pick::{Action, Candidate, PickScreen, Picker, PickerOptions, PreviewFn, source};
use ratatui::text::Line;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Browse the clipboard history; Enter copies the entry, ctrl-x deletes it.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
    /// Initial query.
    #[arg(long, short, default_value = "")]
    query: String,
}

/// What `cb` can do besides browsing.
#[derive(Subcommand)]
enum Cmd {
    /// Record every text copy until stopped (run it as a service).
    Watch,
    /// Record the text on stdin as the newest entry (what `watch` calls on each copy).
    Store,
}

/// Exit code for a failure other than a cancel.
const EXIT_ERROR: u8 = 2;
/// Exit code when the user cancelled; 130 is the shell's code for an interrupt.
const EXIT_CANCEL: u8 = 130;
/// Entries kept; the oldest go first.
const MAX_ENTRIES: usize = 1000;
/// Separates entries in the history file. Text cannot hold a NUL, so it never clashes.
const SEP: char = '\0';

/// One remembered copy.
struct Entry {
    /// Seconds since the epoch when it was copied.
    time: u64,
    /// The copied text.
    text: String,
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
fn parse(data: &str) -> Vec<Entry> {
    data.split(SEP)
        .filter_map(|rec| {
            let (time, text) = rec.split_once('\n')?;
            Some(Entry { time: time.parse().ok()?, text: text.to_string() })
        })
        .collect()
}

/// Writes the entries in the file format.
fn serialize(entries: &[Entry]) -> String {
    entries.iter().map(|e| format!("{}\n{}{SEP}", e.time, e.text)).collect()
}

/// Reads the history; a missing file is an empty history.
fn load() -> Vec<Entry> {
    fs::read_to_string(history_path()).map(|d| parse(&d)).unwrap_or_default()
}

/// Replaces the history file, creating its directory.
fn save(entries: &[Entry]) -> io::Result<()> {
    let path = history_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serialize(entries))
}

/// Adds `text` as the newest entry: an older copy of the same text is dropped, so each text
/// appears once, and the oldest entries beyond the cap go. Blank text is not recorded.
fn push(entries: &mut Vec<Entry>, text: &str, time: u64) {
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

/// One list row for an entry: the text on one line.
fn row(text: &str) -> String {
    text.trim().lines().map(str::trim).collect::<Vec<_>>().join(" ")
}

/// The preview of an entry: its size, then the text as it was copied.
fn preview(text: &str, width: u16, theme: &ui::Theme) -> Vec<Line<'static>> {
    let width = usize::from(width).max(1);
    let mut out = vec![
        Line::styled(
            format!("{} lines, {} characters", text.lines().count(), text.chars().count()),
            ratatui::style::Style::default().fg(theme.colors.muted.into()),
        ),
        Line::default(),
    ];
    for line in text.lines() {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            out.push(Line::default());
        }
        for part in chars.chunks(width) {
            out.push(Line::from(part.iter().collect::<String>()));
        }
    }
    out
}

/// Puts `text` on the clipboard: through `wl-copy` when it exists, else the terminal's OSC 52.
fn copy(text: &str) -> io::Result<()> {
    let spawned = Command::new("wl-copy").stdin(Stdio::piped()).spawn();
    if let Ok(mut child) = spawned {
        child.stdin.take().map(|mut s| s.write_all(text.as_bytes())).transpose()?;
        child.wait()?;
        return Ok(());
    }
    ui::clipboard::copy(text)
}

/// Records the text on stdin.
fn store() -> io::Result<u8> {
    let mut text = String::new();
    io::stdin().read_to_string(&mut text)?;
    let mut entries = load();
    push(&mut entries, &text, now());
    save(&entries)?;
    Ok(0)
}

/// How often `watch` reads the clipboard. Polling, because `wl-paste --watch` needs the
/// data-control protocol, which WSLg's compositor does not offer.
const POLL: Duration = Duration::from_millis(500);

/// The current clipboard text, or `None` when it is empty, not text, or unreadable.
fn paste() -> Option<String> {
    let out = Command::new("wl-paste")
        .args(["--type", "text", "--no-newline"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// Reads the clipboard every `POLL` and records each new text, until stopped.
fn watch() -> io::Result<u8> {
    let mut last = String::new();
    loop {
        if let Some(text) = paste()
            && text != last
        {
            let mut entries = load();
            push(&mut entries, &text, now());
            save(&entries)?;
            last = text;
        }
        std::thread::sleep(POLL);
    }
}

/// Browses the history and copies the chosen entry; ctrl-x deletes it and shows the list again.
fn browse(query: String) -> io::Result<u8> {
    let mut query = query;
    loop {
        let entries = load();
        let wake = Arc::new(AtomicBool::new(true));
        let notify = {
            let wake = wake.clone();
            Arc::new(move || wake.store(true, Ordering::Relaxed)) as Arc<dyn Fn() + Send + Sync>
        };
        let mut picker = Picker::new(
            PickerOptions {
                mode: pick::Mode::Fuzzy,
                paths: false,
                root: std::env::current_dir()?,
                preview: true,
                git: false,
                query: query.clone(),
            },
            notify,
        );
        picker.set_title("Clipboard History");
        picker.set_expect(&["ctrl-x".to_string()]).map_err(io::Error::other)?;
        let f: PreviewFn = Arc::new(preview);
        picker.set_preview_fn(f);
        // Newest first; the whole text is the raw line, so the selection is the entry itself.
        let rows = entries.iter().rev().map(|e| Candidate::with_raw(row(&e.text), e.text.clone()));
        source::push_candidates(&picker.injector(), rows.collect());

        let mut screen = PickScreen::new(picker, wake);
        ui::app::Shell::new("cb")
            .with_navigation(false)
            .with_tabs(false)
            .tick_interval(Duration::from_millis(30))
            .run(&mut screen)?;
        let (key, selection) = match screen.into_result() {
            Action::Accept(sel) => (String::new(), sel),
            Action::AcceptWith(k, sel) => (k, sel),
            _ => return Ok(EXIT_CANCEL),
        };
        let Some(text) = selection.into_iter().next() else {
            return Ok(1);
        };
        if key == "ctrl-x" {
            let mut entries = entries;
            entries.retain(|e| e.text != text);
            save(&entries)?;
            query = String::new();
            continue;
        }
        copy(&text)?;
        return Ok(0);
    }
}

/// Parses the command line and runs the chosen command.
fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Some(Cmd::Watch) => watch(),
        Some(Cmd::Store) => store(),
        None => browse(cli.query),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("cb: {e}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

#[cfg(test)]
mod tests;
