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
use unicode_width::UnicodeWidthChar;

/// Width from which a patch is drawn in two columns, old beside new.
pub const SIDE_BY_SIDE_MIN: u16 = 140;

/// Lines of changed lines marked word by word need at least this much in common (see `git_core::Intraline`).
const MIN_SIMILARITY: f32 = 0.35;

/// Where the files and the hunks start in a drawn patch (indexes into its lines), for jumping between them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Anchors {
    pub files: Vec<usize>,
    pub hunks: Vec<usize>,
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

/// One cell of a drawn code line.
type Cell = (char, Style);

/// The highlighted pieces of each line of a text: `(style, text)`.
type Highlighted = Vec<Vec<(Style, String)>>;

/// Highlight `text` as the language of `path`, line by line.
fn highlight(text: &str, path: &str, theme: &Theme, upto: usize) -> Highlighted {
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
    let mut out = Vec::new();
    // Only as far as the last line a hunk shows: a big file is not highlighted to its end for a change near its top.
    for line in LinesWithEndings::from(text).take(upto) {
        let pieces = hl.highlight_line(line, syntaxes).unwrap_or_default();
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
    let mut push = |c: char, at: usize, style: Style, out: &mut Vec<Cell>| {
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

/// The display width of a cell row.
fn width_of(cells: &[Cell]) -> usize {
    cells.iter().map(|(c, _)| c.width().unwrap_or(0)).sum()
}

/// `cells` as spans, clipped to `width` columns when given and padded to it with `fill` (the row tint).
fn spans_of(cells: &[Cell], width: Option<usize>, fill: Option<Color>) -> Vec<Span<'static>> {
    let mut out: Vec<Span<'static>> = Vec::new();
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
fn hunk_rows(h: &Hunk, file: &FilePatch, old_hl: &Highlighted, new_hl: &Highlighted, theme: &Theme) -> Vec<Row> {
    let c = &theme.colors;
    let (add, del) = (Color::from(c.git.added), Color::from(c.git.deleted));
    let (add_bg, del_bg) = (git::tint(theme, add), git::tint(theme, del));
    let base = Style::new().fg(c.fg.into());

    // Where the hunk's lines sit in the highlighted sides (or in the per-hunk highlight when the file is not kept whole).
    let whole = !file.before.is_empty() || !file.after.is_empty();
    let (hunk_old, hunk_new) = if whole { (Vec::new(), Vec::new()) } else { let (o, n) = hunk_sides(h); (highlight(&o, &file.path, theme, usize::MAX), highlight(&n, &file.path, theme, usize::MAX)) };
    let (mut old_idx, mut new_idx) = if whole { (h.old_start.saturating_sub(1) as usize, h.new_start.saturating_sub(1) as usize) } else { (0, 0) };
    let (mut old_no, mut new_no) = (h.old_start, h.new_start);

    // Word-level marks for each run of removed lines followed by added lines, paired in order.
    let mut marks: Vec<(Vec<Range<usize>>, Vec<Range<usize>>)> = vec![Default::default(); h.lines.len()];
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
        let kind_color = git::kind_color(theme, f.kind);
        let name = match &f.old_path {
            Some(old) => format!("{old} → {}", f.path),
            None => f.path.clone(),
        };
        let mut head = vec![
            Span::styled(format!("{} ", crate::commit::sign(f.kind)), Style::new().fg(kind_color)),
            Span::styled(name, Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD)),
        ];
        if let Some((from, to)) = &f.modes {
            head.push(Span::styled(format!("   mode {from} → {to}"), muted));
        }
        out.push(Line::from(head));
        if let Some(w) = width {
            out.push(Line::from(Span::styled("─".repeat(w.min(120)), muted)));
        }
        if f.binary {
            out.push(Line::from(Span::styled("  binary file not shown", muted)));
            continue;
        }
        let (old_hl, new_hl) = if f.before.is_empty() && f.after.is_empty() { (Vec::new(), Vec::new()) } else { {
            let end = |new: bool| f.hunks.iter().map(|h| if new { h.new_start + h.new_len } else { h.old_start + h.old_len }).max().unwrap_or(0) as usize;
            (highlight(&f.before, &f.path, theme, end(false)), highlight(&f.after, &f.path, theme, end(true)))
        } };
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
            let mut header = vec![Span::styled(format!("@@ -{},{} +{},{} @@", h.old_start, h.old_len, h.new_start, h.new_len), Style::new().fg(c.info.into()))];
            if let Some(ctx) = context {
                header.push(Span::styled(format!(" {ctx}"), muted.add_modifier(Modifier::BOLD)));
            }
            out.push(Line::from(header));
            let rows = hunk_rows(h, f, &old_hl, &new_hl, theme);
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
mod tests {
    use super::*;
    use git_core::{Kind, PatchLine};

    fn patch(old: &str, new: &str) -> FilePatch {
        let hunks = git_core::hunks_of(old.as_bytes(), new.as_bytes());
        FilePatch { path: "src/lib.rs".into(), old_path: None, modes: None, kind: Kind::Modified, binary: false, hunks, before: old.into(), after: new.into() }
    }

    fn text(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()).collect()
    }

    const OLD: &str = "fn main() {\n    let total = count + 1;\n    println!(\"{total}\");\n}\n";
    const NEW: &str = "fn main() {\n    let total = count + step;\n    println!(\"{total}\");\n}\n";

    #[test]
    fn rows_carry_both_line_numbers_and_the_function_context() {
        let opts = Options { width: Some(80), ..Options::default() };
        let (lines, anchors) = render(&[patch(OLD, NEW)], &Theme::default(), &opts, 100);
        let t = text(&lines);
        assert!(t.iter().any(|l| l.starts_with("@@ -1,4 +1,4 @@")), "{t:#?}");
        // A hunk header names the function it sits in when it starts below it.
        let long_old = format!("fn first() {{\n}}\n\nfn second() {{\n{}    let a = 1;\n}}\n", "    // pad\n".repeat(6));
        let long_new = long_old.replace("let a = 1", "let a = 2");
        let (lines, _) = render(&[patch(&long_old, &long_new)], &Theme::default(), &opts, 100);
        assert!(text(&lines).iter().any(|l| l.starts_with("@@") && l.contains("fn second()")), "{:#?}", text(&lines));
        // The removed row shows its old number, the added row its new one.
        assert!(t.iter().any(|l| l.contains(" 2   - ") || l.contains("2   - ")), "{t:#?}");
        assert!(t.iter().any(|l| l.contains("  2 + ")), "{t:#?}");
        assert_eq!((anchors.files.len(), anchors.hunks.len()), (1, 1));
    }

    #[test]
    fn the_changed_word_gets_a_stronger_background_than_the_rest_of_its_row() {
        let opts = Options { width: Some(80), ..Options::default() };
        let (lines, _) = render(&[patch(OLD, NEW)], &Theme::default(), &opts, 100);
        let added = lines.iter().find(|l| l.spans.iter().any(|s| s.content.contains("step"))).expect("the added row");
        let bgs: std::collections::HashSet<_> = added.spans.iter().filter_map(|s| s.style.bg).collect();
        // Two backgrounds: the row tint and the stronger one on `step`.
        assert!(bgs.len() >= 2, "{added:?}");
        let step = added.spans.iter().find(|s| s.content.contains("step")).unwrap();
        let other = added.spans.iter().find(|s| s.content.contains("count")).unwrap();
        assert_ne!(step.style.bg, other.style.bg);
    }

    #[test]
    fn code_is_syntax_highlighted_inside_the_diff() {
        let opts = Options { width: Some(80), ..Options::default() };
        let (lines, _) = render(&[patch(OLD, NEW)], &Theme::default(), &opts, 100);
        let colours: std::collections::HashSet<_> = lines.iter().flat_map(|l| &l.spans).filter_map(|s| s.style.fg).collect();
        assert!(colours.len() >= 5, "{colours:?}");
    }

    #[test]
    fn trailing_whitespace_on_an_added_line_and_conflict_markers_are_called_out() {
        let opts = Options { width: Some(80), ..Options::default() };
        let (lines, _) = render(&[patch("a\nb\n", "a\nb  \n<<<<<<< HEAD\n")], &Theme::default(), &opts, 100);
        let ws = lines.iter().find(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().contains("+ b  ")).expect("the b row");
        let bgs: std::collections::HashSet<_> = ws.spans.iter().filter_map(|s| s.style.bg).collect();
        assert!(bgs.len() >= 2, "{ws:?}");
        let marker = lines.iter().find(|l| l.spans.iter().any(|s| s.content.contains("<<<<<<<"))).expect("the marker row");
        assert!(marker.spans.iter().any(|s| s.content.contains("<<<<<<<") && s.style.add_modifier.contains(Modifier::BOLD)));
    }

    #[test]
    fn renames_and_mode_changes_show_in_the_file_header() {
        let mut p = patch("x\n", "y\n");
        p.kind = Kind::Renamed;
        p.old_path = Some("src/old.rs".into());
        p.modes = Some(("100644".into(), "100755".into()));
        let (lines, _) = render(&[p], &Theme::default(), &Options { width: Some(80), ..Options::default() }, 100);
        let t = text(&lines);
        assert!(t.iter().any(|l| l.contains("src/old.rs → src/lib.rs") && l.contains("mode 100644 → 100755")), "{t:#?}");
    }

    #[test]
    fn wide_screens_draw_old_beside_new() {
        let opts = Options { width: Some(160), ..Options::default() };
        let (lines, _) = render(&[patch(OLD, NEW)], &Theme::default(), &opts, 100);
        let t = text(&lines);
        let pair = t.iter().find(|l| l.contains("count + 1") && l.contains("count + step")).expect("old and new on one row");
        assert!(pair.contains('│'), "{pair}");
        assert!(lines.iter().all(|l| l.spans.iter().map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref())).sum::<usize>() <= 160));
    }

    #[test]
    fn anchors_find_the_next_and_previous_mark() {
        let marks = [3, 10, 20];
        assert_eq!(Anchors::next(&marks, 3), Some(10));
        assert_eq!(Anchors::next(&marks, 20), None);
        assert_eq!(Anchors::prev(&marks, 10), Some(3));
        assert_eq!(Anchors::prev(&marks, 3), None);
        assert_eq!(Anchors { files: vec![1], hunks: vec![2] }.shifted(5), Anchors { files: vec![6], hunks: vec![7] });
    }

    #[test]
    fn a_file_too_big_to_keep_whole_is_still_highlighted_per_hunk() {
        let mut p = patch(OLD, NEW);
        p.before.clear();
        p.after.clear();
        let (lines, _) = render(&[p], &Theme::default(), &Options { width: Some(80), ..Options::default() }, 100);
        let colours: std::collections::HashSet<_> = lines.iter().flat_map(|l| &l.spans).filter_map(|s| s.style.fg).collect();
        assert!(colours.len() >= 5, "{colours:?}");
        let _ = PatchLine { kind: PatchKind::Context, text: String::new() };
    }
}
