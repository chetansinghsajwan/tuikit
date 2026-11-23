use super::cells::{Cell, cells_of};
use super::fold::ignoring_whitespace;
use super::highlight::{highlight, highlight_window};
use crate::git::{self, PatchKind};
use git_core::{FilePatch, Hunk};
use ratatui::style::{Color, Modifier, Style};
use std::ops::Range;
use ui::Theme;

/// The text of a hunk as the old file (context and removed lines) and as the new one (context and added lines).
fn hunk_sides(h: &Hunk) -> (String, String) {
    let (mut old, mut new) = (String::new(), String::new());
    for l in &h.lines {
        if l.kind != PatchKind::Added {
            old.push_str(&l.text);
            old.push('\n');
        }
        if l.kind != PatchKind::Removed {
            new.push_str(&l.text);
            new.push('\n');
        }
    }
    (old, new)
}

/// Whether `line` is the start or the middle of a merge conflict.
fn is_conflict_marker(line: &str) -> bool {
    ["<<<<<<<", "=======", ">>>>>>>", "|||||||"].iter().any(|m| line.starts_with(m))
}

/// Lines of changed lines marked word by word need at least this much in common (see `git_core::Intraline`).
const MIN_SIMILARITY: f32 = 0.35;

/// What one patch line is, ready to draw.
pub(super) struct Row {
    pub(super) kind: PatchKind,
    pub(super) old_no: Option<u32>,
    pub(super) new_no: Option<u32>,
    pub(super) cells: Vec<Cell>,
    /// The row tint.
    pub(super) bg: Option<Color>,
}

/// The rows of one hunk with the syntax and word marks applied.
pub(super) fn hunk_rows(h: &Hunk, file: &FilePatch, theme: &Theme) -> Vec<Row> {
    let c = &theme.colors;
    let (add, del) = (Color::from(c.git.added), Color::from(c.git.deleted));
    let (add_bg, del_bg) = (git::tint(theme, add), git::tint(theme, del));
    let base = Style::new().fg(c.fg.into());

    // Where the hunk's lines sit in the highlighted sides (or in the per-hunk highlight when the file is not kept whole).
    let whole = !file.before.is_empty() || !file.after.is_empty();
    let (old_hl, new_hl) = if whole {
        let (os, ns) =
            (h.old_start.saturating_sub(1) as usize, h.new_start.saturating_sub(1) as usize);
        (
            highlight_window(&file.before, &file.path, theme, os, os + h.old_len as usize),
            highlight_window(&file.after, &file.path, theme, ns, ns + h.new_len as usize),
        )
    } else {
        (Vec::new(), Vec::new())
    };
    let (hunk_old, hunk_new) = if whole {
        (Vec::new(), Vec::new())
    } else {
        let (o, n) = hunk_sides(h);
        (highlight(&o, &file.path, theme, usize::MAX), highlight(&n, &file.path, theme, usize::MAX))
    };
    let (mut old_idx, mut new_idx) = if whole {
        (h.old_start.saturating_sub(1) as usize, h.new_start.saturating_sub(1) as usize)
    } else {
        (0, 0)
    };
    let (mut old_no, mut new_no) = (h.old_start, h.new_start);

    // Word-level marks for each run of removed lines followed by added lines, paired in order.
    /// The word-level marks of one line: the ranges to emphasize on the old and the new side.
    type Marks = (Vec<Range<usize>>, Vec<Range<usize>>);
    let mut marks: Vec<Marks> = vec![Default::default(); h.lines.len()];
    // Pairs that differ only in whitespace: dimmed, and folded into context when whitespace is ignored.
    let mut ws_only = vec![false; h.lines.len()];
    let mut i = 0;
    while i < h.lines.len() {
        if h.lines[i].kind != PatchKind::Removed {
            i += 1;
            continue;
        }
        let rs = i;
        while i < h.lines.len() && h.lines[i].kind == PatchKind::Removed {
            i += 1;
        }
        let as_ = i;
        while i < h.lines.len() && h.lines[i].kind == PatchKind::Added {
            i += 1;
        }
        for k in 0..(as_ - rs).min(i - as_) {
            let (a, b) = (&h.lines[rs + k].text, &h.lines[as_ + k].text);
            if a != b && a.split_whitespace().eq(b.split_whitespace()) {
                ws_only[rs + k] = true;
                ws_only[as_ + k] = true;
                continue;
            }
            let d = git_core::intraline(&h.lines[rs + k].text, &h.lines[as_ + k].text);
            if d.similarity >= MIN_SIMILARITY {
                marks[rs + k].0 = d.old;
                marks[as_ + k].1 = d.new;
            }
        }
    }

    let mut rows = Vec::new();
    for (k, l) in h.lines.iter().enumerate() {
        let old_side = |idx: usize| if whole { old_hl.get(idx) } else { hunk_old.get(idx) };
        let new_side = |idx: usize| if whole { new_hl.get(idx) } else { hunk_new.get(idx) };
        let conflict = is_conflict_marker(&l.text);
        let (kind, o_no, n_no, pieces, bg, strong, strong_bg) = match l.kind {
            PatchKind::Context => {
                let r = (
                    PatchKind::Context,
                    Some(old_no),
                    Some(new_no),
                    new_side(new_idx),
                    None,
                    &[][..],
                    del_bg,
                );
                old_no += 1;
                new_no += 1;
                old_idx += 1;
                new_idx += 1;
                r
            }
            PatchKind::Removed => {
                let r = (
                    PatchKind::Removed,
                    Some(old_no),
                    None,
                    old_side(old_idx),
                    Some(del_bg),
                    &marks[k].0[..],
                    ui::mix(del, c.bg.into(), 42),
                );
                old_no += 1;
                old_idx += 1;
                r
            }
            PatchKind::Added => {
                let r = (
                    PatchKind::Added,
                    None,
                    Some(new_no),
                    new_side(new_idx),
                    Some(add_bg),
                    &marks[k].1[..],
                    ui::mix(add, c.bg.into(), 42),
                );
                new_no += 1;
                new_idx += 1;
                r
            }
        };
        // A change in whitespace only is dimmed, or shown as an unchanged line when whitespace is ignored.
        let ws = ws_only[k];
        let ignore = ws && ignoring_whitespace();
        if ignore && kind == PatchKind::Removed {
            continue;
        }
        let (kind, bg, strong, pieces) = match (ws, ignore) {
            (true, true) => (PatchKind::Context, None, &[][..], None),
            (true, false) => (kind, Some(ui::mix(c.muted.into(), c.bg.into(), 14)), &[][..], None),
            _ => (kind, bg, strong, pieces),
        };
        let base = if ws { Style::new().fg(c.muted.into()) } else { base };
        let mut cells = if conflict {
            cells_of(
                &l.text,
                None,
                &[],
                bg,
                strong_bg,
                Style::new().fg(c.warning.into()).add_modifier(Modifier::BOLD),
            )
        } else {
            cells_of(&l.text, pieces.map(Vec::as_slice), strong, bg, strong_bg, base)
        };
        // Trailing whitespace on an added line is an error in most projects: show it.
        if kind == PatchKind::Added {
            let trimmed = l.text.trim_end().chars().count();
            let tail = l.text.chars().count() - trimmed;
            if tail > 0 && !l.text.contains('\t') {
                let at = cells.len().saturating_sub(tail);
                for cell in &mut cells[at..] {
                    cell.1 = cell.1.bg(ui::mix(c.error.into(), c.bg.into(), 55));
                }
            }
        }
        rows.push(Row { kind, old_no: o_no, new_no: n_no, cells, bg });
    }
    rows
}
