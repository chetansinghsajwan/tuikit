//! Patches drawn like `delta` draws them: a header per file (renames and mode changes included), hunk headers
//! with the function they sit in, both line numbers, syntax-highlighted code over a faint red or green row,
//! the changed words of a changed line marked stronger, trailing whitespace on added lines and conflict markers
//! called out, and a side-by-side layout when there is room.

use crate::git::{self, PatchKind};
use crate::text::{syntax_set, syntax_theme};
use crate::Options;
use git_core::{FilePatch, Hunk};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::ops::Range;
use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;
use syntect::util::LinesWithEndings;
use ui::Theme;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Width from which a patch is drawn in two columns, old beside new.
pub const SIDE_BY_SIDE_MIN: u16 = 140;

/// Lines of changed lines marked word by word need at least this much in common (see `git_core::Intraline`).
const MIN_SIMILARITY: f32 = 0.35;

/// Where the files and the hunks start in a drawn patch (indexes into its lines), for jumping between them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Anchors {
    pub files: Vec<usize>,
    pub hunks: Vec<usize>,
    /// The path of each file in `files`.
    pub paths: Vec<String>,
}

impl Anchors {
    /// The first anchor after row `at` in `marks` (sorted), if any.
    pub fn next(marks: &[usize], at: usize) -> Option<usize> {
        marks.iter().copied().find(|&m| m > at)
    }

    /// The last anchor before row `at` in `marks` (sorted), if any.
    pub fn prev(marks: &[usize], at: usize) -> Option<usize> {
        marks.iter().copied().rev().find(|&m| m < at)
    }

    /// Everything shifted down by `n` rows (the patch sits under other lines).
    pub fn shifted(mut self, n: usize) -> Anchors {
        self.files.iter_mut().for_each(|m| *m += n);
        self.hunks.iter_mut().for_each(|m| *m += n);
        self
    }
}

thread_local! {
    /// Files the reader folded or unfolded by hand (a file that folds by itself, a lockfile say, is in here when opened).
    static TOGGLED: std::cell::RefCell<std::collections::HashSet<String>> = std::cell::RefCell::new(Default::default());
    static IGNORE_WHITESPACE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Fold the file `path` of the patch, or unfold it.
pub fn toggle_fold(path: &str) {
    TOGGLED.with(|t| {
        let mut t = t.borrow_mut();
        if !t.remove(path) {
            t.insert(path.to_string());
        }
    });
}

/// Forget the hand-made folds (another commit is shown).
pub fn reset_folds() {
    TOGGLED.with(|t| t.borrow_mut().clear());
}

/// Show changes that only differ in whitespace as unchanged lines.
pub fn set_ignore_whitespace(on: bool) {
    IGNORE_WHITESPACE.with(|w| w.set(on));
}

pub fn ignoring_whitespace() -> bool {
    IGNORE_WHITESPACE.with(|w| w.get())
}

/// Files nobody reads line by line: lockfiles, minified and generated output, and anything that changes this much.
const BIG_FILE: usize = 400;

fn folds_by_itself(f: &FilePatch, changed: usize) -> bool {
    let name = f.path.rsplit('/').next().unwrap_or(&f.path);
    let generated = ["Cargo.lock", "package-lock.json", "yarn.lock", "pnpm-lock.yaml", "flake.lock", "poetry.lock", "go.sum", "composer.lock", "Gemfile.lock"].contains(&name) || name.ends_with(".min.js") || name.ends_with(".min.css") || name.ends_with(".map");
    generated || changed > BIG_FILE
}

/// Whether the file is drawn as its header only.
pub fn is_folded(f: &FilePatch, changed: usize) -> bool {
    folds_by_itself(f, changed) != TOGGLED.with(|t| t.borrow().contains(&f.path))
}

/// Added and removed lines of a file.
pub fn counts(f: &FilePatch) -> (usize, usize) {
    let (mut a, mut r) = (0, 0);
    for l in f.hunks.iter().flat_map(|h| &h.lines) {
        match l.kind {
            PatchKind::Added => a += 1,
            PatchKind::Removed => r += 1,
            PatchKind::Context => {}
        }
    }
    (a, r)
}

/// One cell of a drawn code line.
type Cell = (char, Style);

/// The highlighted pieces of each line of a text: `(style, text)`.
type Highlighted = Vec<Vec<(Style, String)>>;

/// Highlight `text` as the language of `path`, line by line.
fn highlight(text: &str, path: &str, theme: &Theme, upto: usize) -> Highlighted {
    highlight_window(text, path, theme, 0, upto)
}

/// How many lines before a window are parsed (and not kept) so the window starts in the right state.
const LEAD_IN: usize = 10;

/// Lines longer than this are not highlighted (Markdown prose and minified files take the highlighter milliseconds a line).
const LONG_LINE: usize = 240;

/// How many lines one rendering highlights at most; the rest of a huge diff is drawn plain.
const HIGHLIGHT_BUDGET: usize = 1500;

thread_local! {
    static BUDGET: std::cell::Cell<usize> = const { std::cell::Cell::new(HIGHLIGHT_BUDGET) };
}

/// Highlight lines `from..upto` of `text`: the lines before are only parsed from `LEAD_IN` lines back, and come
/// out empty. Highlighting is by far the slowest part of a diff, so it covers what a hunk shows and no more.
fn highlight_window(text: &str, path: &str, theme: &Theme, from: usize, upto: usize) -> Highlighted {
    let start = from.saturating_sub(LEAD_IN);
    let syntaxes = syntax_set();
    let name = std::path::Path::new(path);
    let syntax = syntaxes
        .find_syntax_for_file(name)
        .ok()
        .flatten()
        .or_else(|| text.lines().next().and_then(|l| syntaxes.find_syntax_by_first_line(l)))
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let theme = syntax_theme(theme);
    let mut hl = HighlightLines::new(syntax, &theme);
    let mut out: Vec<Vec<(Style, String)>> = vec![Vec::new(); start];
    for (n, line) in LinesWithEndings::from(text).enumerate().skip(start).take(upto.saturating_sub(start)) {
        let spent = BUDGET.with(|b| { let left = b.get(); b.set(left.saturating_sub(1)); left == 0 });
        if spent || line.len() > LONG_LINE {
            out.push(Vec::new());
            continue;
        }
        let pieces = hl.highlight_line(line, syntaxes).unwrap_or_default();
        if n < from {
            out.push(Vec::new());
            continue;
        }
        out.push(
            pieces
                .into_iter()
                .map(|(st, piece)| {
                    let mut style = Style::new().fg(Color::Rgb(st.foreground.r, st.foreground.g, st.foreground.b));
                    if st.font_style.contains(FontStyle::BOLD) {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if st.font_style.contains(FontStyle::ITALIC) {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                    (style, piece.trim_end_matches(['\n', '\r']).to_string())
                })
                .collect(),
        );
    }
    out
}

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

/// The line a hunk sits in, like git's `@@ ... @@ fn name`: the nearest line above `from` (0-based) that starts a
/// function, a type, a section, or any line starting at column 0 with a letter, `_` or `$`.
fn context_of(lines: &[&str], from: usize) -> Option<String> {
    const KEYWORDS: [&str; 14] = ["fn ", "pub fn ", "async fn ", "def ", "class ", "func ", "function ", "impl ", "struct ", "enum ", "trait ", "interface ", "public ", "private "];
    let upto = from.min(lines.len());
    for line in lines[..upto].iter().rev() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let top = line.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$' || c == '#');
        if top || KEYWORDS.iter().any(|k| trimmed.starts_with(k)) {
            let mut s: String = trimmed.trim_end().chars().take(60).collect();
            if trimmed.chars().count() > 60 {
                s.push('…');
            }
            return Some(s);
        }
    }
    None
}

/// The cells of one code line: the syntax colours of `pieces`, the `strong` byte ranges on a stronger background,
/// tabs spread over four columns; `bg` is the row tint.
fn cells_of(text: &str, pieces: Option<&[(Style, String)]>, strong: &[Range<usize>], bg: Option<Color>, strong_bg: Color, base: Style) -> Vec<Cell> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    let push = |c: char, at: usize, style: Style, out: &mut Vec<Cell>| {
        let mut style = style;
        if let Some(bg) = bg {
            style = style.bg(if strong.iter().any(|r| r.contains(&at)) { strong_bg } else { bg });
        }
        if c == '\t' {
            out.extend(std::iter::repeat_n((' ', style), 4));
        } else {
            out.push((c, style));
        }
    };
    match pieces.filter(|p| p.iter().map(|(_, t)| t.len()).sum::<usize>() == text.len()) {
        Some(pieces) => {
            for (style, piece) in pieces {
                for (i, c) in piece.char_indices() {
                    push(c, offset + i, *style, &mut out);
                }
                offset += piece.len();
            }
        }
        None => {
            for (i, c) in text.char_indices() {
                push(c, i, base, &mut out);
            }
        }
    }
    out
}

/// `cells` as spans, clipped to `width` columns when given and padded to it with `fill` (the row tint).
fn spans_of(cells: &[Cell], width: Option<usize>, fill: Option<Color>) -> Vec<Span<'static>> {
    let mut out: Vec<Span<'static>> = Vec::new();
    // A line that does not fit ends in a marker instead of being cut silently.
    let total: usize = cells.iter().map(|(c, _)| c.width().unwrap_or(0)).sum();
    let clipped = width.is_some_and(|max| total > max && max > 1);
    let width = if clipped { width.map(|m| m - 1) } else { width };
    let mut used = 0;
    let mut run = String::new();
    let mut run_style: Option<Style> = None;
    let flush = |run: &mut String, style: &mut Option<Style>, out: &mut Vec<Span<'static>>| {
        if let Some(s) = style.take() {
            out.push(Span::styled(std::mem::take(run), s));
        }
    };
    for (c, style) in cells {
        let w = c.width().unwrap_or(0);
        if width.is_some_and(|max| used + w > max) {
            break;
        }
        if run_style != Some(*style) {
            flush(&mut run, &mut run_style, &mut out);
            run_style = Some(*style);
        }
        run.push(*c);
        used += w;
    }
    flush(&mut run, &mut run_style, &mut out);
    if clipped {
        let mut s = Style::new().fg(Color::DarkGray);
        if let Some(bg) = fill {
            s = s.bg(bg);
        }
        out.push(Span::styled("…", s));
        return out;
    }
    if let (Some(max), Some(bg)) = (width, fill) {
        if used < max {
            out.push(Span::styled(" ".repeat(max - used), Style::new().bg(bg)));
        }
    }
    out
}

/// What one patch line is, ready to draw.
struct Row {
    kind: PatchKind,
    old_no: Option<u32>,
    new_no: Option<u32>,
    cells: Vec<Cell>,
    /// The row tint.
    bg: Option<Color>,
}

/// The rows of one hunk with the syntax and word marks applied.
fn hunk_rows(h: &Hunk, file: &FilePatch, theme: &Theme) -> Vec<Row> {
    let c = &theme.colors;
    let (add, del) = (Color::from(c.git.added), Color::from(c.git.deleted));
    let (add_bg, del_bg) = (git::tint(theme, add), git::tint(theme, del));
    let base = Style::new().fg(c.fg.into());

    // Where the hunk's lines sit in the highlighted sides (or in the per-hunk highlight when the file is not kept whole).
    let whole = !file.before.is_empty() || !file.after.is_empty();
    let (old_hl, new_hl) = if whole {
        let (os, ns) = (h.old_start.saturating_sub(1) as usize, h.new_start.saturating_sub(1) as usize);
        (highlight_window(&file.before, &file.path, theme, os, os + h.old_len as usize), highlight_window(&file.after, &file.path, theme, ns, ns + h.new_len as usize))
    } else {
        (Vec::new(), Vec::new())
    };
    let (hunk_old, hunk_new) = if whole { (Vec::new(), Vec::new()) } else { let (o, n) = hunk_sides(h); (highlight(&o, &file.path, theme, usize::MAX), highlight(&n, &file.path, theme, usize::MAX)) };
    let (mut old_idx, mut new_idx) = if whole { (h.old_start.saturating_sub(1) as usize, h.new_start.saturating_sub(1) as usize) } else { (0, 0) };
    let (mut old_no, mut new_no) = (h.old_start, h.new_start);

    // Word-level marks for each run of removed lines followed by added lines, paired in order.
    let mut marks: Vec<(Vec<Range<usize>>, Vec<Range<usize>>)> = vec![Default::default(); h.lines.len()];
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
                let r = (PatchKind::Context, Some(old_no), Some(new_no), new_side(new_idx), None, &[][..], del_bg);
                old_no += 1;
                new_no += 1;
                old_idx += 1;
                new_idx += 1;
                r
            }
            PatchKind::Removed => {
                let r = (PatchKind::Removed, Some(old_no), None, old_side(old_idx), Some(del_bg), &marks[k].0[..], ui::mix(del, c.bg.into(), 42));
                old_no += 1;
                old_idx += 1;
                r
            }
            PatchKind::Added => {
                let r = (PatchKind::Added, None, Some(new_no), new_side(new_idx), Some(add_bg), &marks[k].1[..], ui::mix(add, c.bg.into(), 42));
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
            cells_of(&l.text, None, &[], bg, strong_bg, Style::new().fg(c.warning.into()).add_modifier(Modifier::BOLD))
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

/// The drawn patch of `files`: lines (at most `limit`) and where the files and hunks start.
pub fn render(files: &[FilePatch], theme: &Theme, opts: &Options, limit: usize) -> (Vec<Line<'static>>, Anchors) {
    BUDGET.with(|b| b.set(HIGHLIGHT_BUDGET));
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
            out.push(Line::from(Span::styled(format!("  {} lines changed, folded (fold key opens it)", added + removed), muted)));
            continue;
        }
        let after_lines: Vec<&str> = f.after.lines().collect();
        let before_lines: Vec<&str> = f.before.lines().collect();
        for h in &f.hunks {
            if out.len() >= limit {
                return (out, anchors);
            }
            anchors.hunks.push(out.len());
            let context = if !after_lines.is_empty() {
                context_of(&after_lines, h.new_start.saturating_sub(1) as usize)
            } else {
                context_of(&before_lines, h.old_start.saturating_sub(1) as usize)
            };
            // Where the change is, by name when there is one: the numbers are in the gutter below.
            let mut header = vec![Span::styled(format!("  ⋯ line {}", h.new_start.max(1)), Style::new().fg(c.info.into()))];
            if let Some(ctx) = context {
                header.push(Span::styled(format!("  {ctx}"), Style::new().fg(c.fg.into()).add_modifier(Modifier::BOLD)));
            }
            out.push(Line::from(header));
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

/// A file's header: one tinted bar across the pane with the fold mark, the state letter, the folder muted and the
/// file name bold (`old → new` for a rename), and the counts at the right edge, so files stand out while scrolling.
pub(crate) fn file_bar(f: &FilePatch, folded: bool, added: usize, removed: usize, width: Option<usize>, theme: &Theme) -> Line<'static> {
    let c = &theme.colors;
    let bar_bg = ui::mix(c.accent.into(), c.bg.into(), 16);
    let kind_color = git::kind_color(theme, f.kind);
    let muted = Style::new().fg(c.muted.into());
    let (dir, file) = match f.path.rsplit_once('/') {
        Some((d, n)) => (format!("{d}/"), n.to_string()),
        None => (String::new(), f.path.clone()),
    };
    let mut spans = vec![
        Span::styled(if folded { "▸ " } else { "▾ " }, muted),
        Span::styled(format!("{} ", crate::commit::sign(f.kind)), Style::new().fg(kind_color).add_modifier(Modifier::BOLD)),
    ];
    if let Some(old) = &f.old_path {
        spans.push(Span::styled(format!("{old} → "), muted));
    }
    spans.push(Span::styled(dir, muted));
    spans.push(Span::styled(file, Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD)));
    if let Some((from, to)) = &f.modes {
        spans.push(Span::styled(format!("   mode {from} → {to}"), muted));
    }
    let counts = if f.binary {
        vec![Span::styled("binary", muted)]
    } else {
        vec![Span::styled(format!("+{added}"), Style::new().fg(c.git.added.into())), Span::raw(" "), Span::styled(format!("−{removed}"), Style::new().fg(c.git.deleted.into()))]
    };
    let used: usize = spans.iter().chain(counts.iter()).map(|s| s.content.width()).sum();
    if let Some(w) = width {
        spans.push(Span::raw(" ".repeat(w.saturating_sub(used + 1).max(1))));
    } else {
        spans.push(Span::raw("  "));
    }
    spans.extend(counts);
    for s in &mut spans {
        s.style = s.style.bg(bar_bg);
    }
    Line::from(spans)
}

/// One row of the unified view: both line numbers, the sign, the code, the tint to the edge.
fn unified_row(r: &Row, digits: usize, width: Option<usize>, theme: &Theme) -> Line<'static> {
    let c = &theme.colors;
    let num = |n: Option<u32>| match n {
        Some(n) => format!("{n:>digits$}"),
        None => " ".repeat(digits),
    };
    let (sign, color) = match r.kind {
        PatchKind::Context => (' ', c.fg),
        PatchKind::Added => ('+', c.git.added),
        PatchKind::Removed => ('-', c.git.deleted),
    };
    let mut spans = vec![
        Span::styled(format!("{} {} ", num(r.old_no), num(r.new_no)), Style::new().fg(c.muted.into())),
        Span::styled(format!("{sign} "), Style::new().fg(color.into())),
    ];
    let prefix = 2 * digits + 2 + 2;
    spans.extend(spans_of(&r.cells, width.map(|w| w.saturating_sub(prefix)), r.bg));
    if let Some(bg) = r.bg {
        // The gutter and the sign carry the tint too.
        for s in spans.iter_mut().take(2) {
            s.style = s.style.bg(bg);
        }
    }
    Line::from(spans)
}

/// A hunk side by side: removed lines on the left, added on the right, context on both, rows of a change paired in order.
fn side_rows(rows: &[Row], digits: usize, width: usize, theme: &Theme, out: &mut Vec<Line<'static>>, limit: usize) {
    let c = &theme.colors;
    let half = width.saturating_sub(1) / 2;
    let code_w = half.saturating_sub(digits + 3);
    let blank = |bg: Option<Color>| {
        let mut s = Span::raw(" ".repeat(half));
        if let Some(bg) = bg {
            s.style = Style::new().bg(bg);
        }
        vec![s]
    };
    let side = |r: Option<&Row>, number: fn(&Row) -> Option<u32>| -> Vec<Span<'static>> {
        let Some(r) = r else { return blank(None) };
        let n = match number(r) {
            Some(n) => format!("{n:>digits$} "),
            None => " ".repeat(digits + 1),
        };
        let (sign, color) = match r.kind {
            PatchKind::Context => (' ', c.fg),
            PatchKind::Added => ('+', c.git.added),
            PatchKind::Removed => ('-', c.git.deleted),
        };
        let mut head = vec![Span::styled(n, Style::new().fg(c.muted.into())), Span::styled(format!("{sign} "), Style::new().fg(color.into()))];
        if let Some(bg) = r.bg {
            head.iter_mut().for_each(|s| s.style = s.style.bg(bg));
        }
        head.extend(spans_of(&r.cells, Some(code_w), r.bg));
        head
    };
    let sep = Span::styled("│", Style::new().fg(c.border.into()));
    let mut i = 0;
    while i < rows.len() && out.len() < limit {
        if rows[i].kind == PatchKind::Context {
            let mut line = side(Some(&rows[i]), |r| r.old_no);
            line.push(sep.clone());
            line.extend(side(Some(&rows[i]), |r| r.new_no));
            out.push(Line::from(line));
            i += 1;
            continue;
        }
        let rs = i;
        while i < rows.len() && rows[i].kind == PatchKind::Removed {
            i += 1;
        }
        let as_ = i;
        while i < rows.len() && rows[i].kind == PatchKind::Added {
            i += 1;
        }
        for k in 0..(as_ - rs).max(i - as_) {
            if out.len() >= limit {
                return;
            }
            let left = rows.get(rs + k).filter(|_| rs + k < as_);
            let right = rows.get(as_ + k).filter(|_| as_ + k < i);
            let mut line = side(left, |r| r.old_no);
            line.push(sep.clone());
            line.extend(side(right, |r| r.new_no));
            out.push(Line::from(line));
        }
    }
}

#[cfg(test)]
mod tests;
