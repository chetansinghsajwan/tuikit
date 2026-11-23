//! Patches drawn like `delta` draws them: a header per file (renames and mode changes included), hunk headers
//! with the function they sit in, both line numbers, syntax-highlighted code over a faint red or green row,
//! the changed words of a changed line marked stronger, trailing whitespace on added lines and conflict markers
//! called out, and a side-by-side layout when there is room.

mod anchors;
mod bar;
mod cells;
mod fold;
mod header;
mod highlight;
mod inline;
mod rows;
mod side;
mod unified;

pub use anchors::Anchors;
pub(crate) use bar::file_bar;
pub use fold::{
    counts, ignoring_whitespace, is_folded, reset_folds, set_ignore_whitespace, toggle_fold,
};
pub use unified::render_unified;

use crate::Options;
use git_core::FilePatch;
use header::hunk_header;
use inline::unified_row;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use rows::hunk_rows;
use side::side_rows;
use ui::Theme;

/// Width from which a patch is drawn in two columns, old beside new.
pub const SIDE_BY_SIDE_MIN: u16 = 140;

/// The drawn patch of `files`: lines (at most `limit`) and where the files and hunks start.
pub fn render(
    files: &[FilePatch],
    theme: &Theme,
    opts: &Options,
    limit: usize,
) -> (Vec<Line<'static>>, Anchors) {
    highlight::reset_budget();
    let c = &theme.colors;
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut anchors = Anchors::default();
    let digits = files
        .iter()
        .flat_map(|f| &f.hunks)
        .map(|h| h.old_start.max(h.new_start) + h.old_len.max(h.new_len))
        .max()
        .unwrap_or(1)
        .to_string()
        .len();
    let muted = Style::new().fg(c.muted.into());
    let width = opts.width.map(usize::from);
    let side_by_side = opts.width.is_some_and(|w| w >= SIDE_BY_SIDE_MIN);

    for f in files {
        if out.len() >= limit {
            break;
        }
        // File header: the state sign, the path (`old → new` for a rename), the mode change, a rule.
        out.push(Line::default());
        anchors.files.push(out.len());
        anchors.paths.push(f.path.clone());
        let (added, removed) = counts(f);
        let folded = !f.binary && is_folded(f, added + removed);
        out.push(file_bar(f, folded, added, removed, width, theme));
        if f.binary {
            out.push(Line::from(Span::styled("  binary file not shown", muted)));
            continue;
        }
        if folded {
            out.push(Line::from(Span::styled(
                format!("  {} lines changed, folded (fold key opens it)", added + removed),
                muted,
            )));
            continue;
        }
        let after_lines: Vec<&str> = f.after.lines().collect();
        let before_lines: Vec<&str> = f.before.lines().collect();
        for h in &f.hunks {
            if out.len() >= limit {
                return (out, anchors);
            }
            anchors.hunks.push(out.len());
            out.push(hunk_header(h, &before_lines, &after_lines, theme));
            let rows = hunk_rows(h, f, theme);
            if side_by_side {
                side_rows(&rows, digits, width.unwrap_or(0), theme, &mut out, limit);
            } else {
                for r in &rows {
                    if out.len() >= limit {
                        return (out, anchors);
                    }
                    out.push(unified_row(r, digits, width, theme));
                }
            }
        }
    }
    (out, anchors)
}

#[cfg(test)]
mod tests;
