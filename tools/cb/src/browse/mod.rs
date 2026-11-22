//! The browser: lists the history in a fuzzy picker and puts the chosen entry back on the
//! clipboard.

use crate::backend::Backend;
use crate::store;
use pick::{Action, Candidate, PickScreen, Picker, PickerOptions, PreviewFn, source};
use ratatui::text::Line;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Exit code when the user cancelled; 130 is the shell's code for an interrupt.
const EXIT_CANCEL: u8 = 130;

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

/// Browses the history and copies the chosen entry; ctrl-x deletes it and shows the list again.
pub fn run(query: String, backend: &dyn Backend) -> io::Result<u8> {
    let mut query = query;
    loop {
        let entries = store::load();
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
            store::remove(&text)?;
            query = String::new();
            continue;
        }
        backend.write(&text)?;
        return Ok(0);
    }
}

#[cfg(test)]
mod tests;
