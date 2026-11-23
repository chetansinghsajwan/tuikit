use super::cells::spans_of;
use super::rows::Row;
use crate::git::PatchKind;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ui::Theme;

/// A hunk side by side: removed lines on the left, added on the right, context on both, rows of a change paired in order.
pub(super) fn side_rows(
    rows: &[Row],
    digits: usize,
    width: usize,
    theme: &Theme,
    out: &mut Vec<Line<'static>>,
    limit: usize,
) {
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
        let mut head = vec![
            Span::styled(n, Style::new().fg(c.muted.into())),
            Span::styled(format!("{sign} "), Style::new().fg(color.into())),
        ];
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
