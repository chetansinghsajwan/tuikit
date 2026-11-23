use ratatui::style::{Color, Style};
use ratatui::text::Span;
use std::ops::Range;
use unicode_width::UnicodeWidthChar;

/// One cell of a drawn code line.
pub(super) type Cell = (char, Style);

/// The cells of one code line: the syntax colours of `pieces`, the `strong` byte ranges on a stronger background,
/// tabs spread over four columns; `bg` is the row tint.
pub(super) fn cells_of(
    text: &str,
    pieces: Option<&[(Style, String)]>,
    strong: &[Range<usize>],
    bg: Option<Color>,
    strong_bg: Color,
    base: Style,
) -> Vec<Cell> {
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
pub(super) fn spans_of(
    cells: &[Cell],
    width: Option<usize>,
    fill: Option<Color>,
) -> Vec<Span<'static>> {
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
    if let (Some(max), Some(bg)) = (width, fill)
        && used < max
    {
        out.push(Span::styled(" ".repeat(max - used), Style::new().bg(bg)));
    }
    out
}
