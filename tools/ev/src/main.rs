//! The `ev` binary: browse the environment variables of the current process and print the chosen `NAME=value` lines. It is a picker-style tool on the shared `pick` library; it is
//! its own tool so that editing, origins and the like can grow here later.

use clap::Parser;
use pick::{Action, Candidate, PickScreen, Picker, PickerOptions, PreviewFn, source};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::collections::HashSet;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use ui::Theme;

/// Browse the environment variables. The selection is printed as `NAME=value`, one per line.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Show the values of secret-looking variables (`*TOKEN*`, `*SECRET*`, `*PASSWORD*`, `*KEY*`).
    #[arg(long)]
    reveal: bool,
    /// Initial query.
    #[arg(long, short, default_value = "")]
    query: String,
}

/// Exit code when the picker was accepted with nothing selected.
const EXIT_NO_SELECTION: u8 = 1;
/// Exit code when the user cancelled; 130 is the shell's code for an interrupt.
const EXIT_CANCEL: u8 = 130;
/// Name parts that mark a variable as holding a secret.
const SECRET_PARTS: [&str; 6] = ["TOKEN", "SECRET", "PASSWORD", "PASSWD", "CREDENTIAL", "KEY"];
/// Variables that hold a list of directories although their name does not end in `PATH`.
const DIR_LISTS: [&str; 2] = ["XDG_DATA_DIRS", "XDG_CONFIG_DIRS"];

/// Whether the name looks like it holds a secret, so its value is hidden by default.
fn is_secret(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_PARTS.iter().any(|p| upper.contains(p))
}

/// Whether the value is a `:`-separated list of directories, so it reads better one entry per row.
fn is_dir_list(name: &str, value: &str) -> bool {
    if name.ends_with("PATH") || DIR_LISTS.contains(&name) {
        return true;
    }
    value.contains(':') && value.split(':').all(|e| e.is_empty() || e.starts_with('/'))
}

/// The variables of this process, sorted by name, as candidates. A row shows only the name, and
/// the query matches names only; the candidate keeps the full `NAME=value` line, which is what is
/// printed and what the preview reads (it hides a secret's value there).
fn candidates(vars: impl Iterator<Item = (String, String)>) -> Vec<Candidate> {
    let mut vars: Vec<_> = vars.collect();
    vars.sort();
    vars.iter()
        .map(|(name, value)| Candidate::with_raw(name.clone(), format!("{name}={value}")))
        .collect()
}

/// Splits `text` into pieces of at most `width` characters, so a long value wraps in the pane.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars.chunks(width).map(|c| c.iter().collect()).collect()
}

/// The preview of one `NAME=value` line: its size, then the value, wrapped; a directory list
/// goes one entry per row and flags the entries that do not exist and the ones listed twice.
fn preview(line: &str, width: u16, theme: &Theme, reveal: bool) -> Vec<Line<'static>> {
    let muted = Style::new().fg(theme.colors.muted.into());
    let (name, value) = line.split_once('=').unwrap_or((line, ""));
    let mut out = Vec::new();
    if is_secret(name) && !reveal {
        out.push(Line::from(Span::styled(
            format!("{} chars, hidden (secret name; run `ev --reveal` to show)", value.chars().count()),
            muted,
        )));
        return out;
    }
    if !is_dir_list(name, value) {
        out.push(Line::from(Span::styled(format!("{} chars", value.chars().count()), muted)));
        out.push(Line::default());
        for part in value.split('\n') {
            for piece in wrap(part, usize::from(width)) {
                out.push(Line::from(piece));
            }
        }
        return out;
    }
    let entries: Vec<&str> = value.split(':').collect();
    out.push(Line::from(Span::styled(format!("{} entries", entries.len()), muted)));
    out.push(Line::default());
    let mut seen = HashSet::new();
    for (i, entry) in entries.iter().enumerate() {
        let (flag, color) = if entry.is_empty() {
            ("empty (current directory)", theme.colors.warning)
        } else if !entry.starts_with('/') {
            ("relative", theme.colors.warning)
        } else if !Path::new(entry).exists() {
            ("missing", theme.colors.error)
        } else if !seen.insert(*entry) {
            ("duplicate", theme.colors.warning)
        } else {
            ("", theme.colors.fg)
        };
        let mut spans = vec![
            Span::styled(format!("{:>2}  ", i + 1), muted),
            Span::raw(if entry.is_empty() { "-".to_string() } else { entry.to_string() }),
        ];
        if !flag.is_empty() {
            spans.push(Span::styled(
                format!("  {flag}"),
                Style::new().fg(color.into()).add_modifier(Modifier::ITALIC),
            ));
        }
        out.push(Line::from(spans));
    }
    out
}

/// Parses the command line, runs the picker and maps a failure to exit code 2.
fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("ev: {e}");
            ExitCode::from(2)
        }
    }
}

/// Lists the variables in the picker on `/dev/tty` and prints the selection to stdout. It never
/// opens a popup: a popup is another process and would show the popup's environment, not this one.
fn run(cli: Cli) -> io::Result<u8> {
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
            query: cli.query,
        },
        notify,
    );
    picker.set_title("Environment");
    let reveal = cli.reveal;
    let f: PreviewFn = Arc::new(move |line, width, theme| preview(line, width, theme, reveal));
    picker.set_preview_fn(f);
    let vars = std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v.to_string_lossy().into_owned()));
    source::push_candidates(&picker.injector(), candidates(vars));

    let mut screen = PickScreen::new(picker, wake);
    ui::app::Shell::new("ev")
        .with_navigation(false)
        .with_tabs(false)
        .tick_interval(Duration::from_millis(30))
        .run(&mut screen)?;
    let selection = match screen.into_result() {
        Action::Accept(sel) | Action::AcceptWith(_, sel) => sel,
        _ => return Ok(EXIT_CANCEL),
    };
    if selection.is_empty() {
        return Ok(EXIT_NO_SELECTION);
    }
    let mut out = io::stdout().lock();
    for s in selection {
        writeln!(out, "{s}")?;
    }
    Ok(0)
}

#[cfg(test)]
mod tests;
