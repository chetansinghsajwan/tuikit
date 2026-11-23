use super::cells::spans_of;
use super::rows::Row;
use crate::git::PatchKind;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ui::Theme;

/// One row of the unified view: both line numbers, the sign, the code, the tint to the edge.
pub(super) fn unified_row(
    r: &Row,
    digits: usize,
    width: Option<usize>,
    theme: &Theme,
) -> Line<'static> {
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
        Span::styled(
            format!("{} {} ", num(r.old_no), num(r.new_no)),
            Style::new().fg(c.muted.into()),
        ),
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
