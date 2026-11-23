use crate::git;
use git_core::FilePatch;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ui::Theme;
use unicode_width::UnicodeWidthStr;

/// A file's header: one tinted bar across the pane with the fold mark, the state letter, the folder muted and the
/// file name bold (`old → new` for a rename), and the counts at the right edge, so files stand out while scrolling.
pub(crate) fn file_bar(
    f: &FilePatch,
    folded: bool,
    added: usize,
    removed: usize,
    width: Option<usize>,
    theme: &Theme,
) -> Line<'static> {
    let c = &theme.colors;
    let bar_bg = ui::mix(c.accent.into(), c.bg.into(), 16);
    let kind_color = git::kind_color(theme, f.kind);
    let muted = Style::new().fg(c.muted.into());
    let (dir, file) = match f.path.rsplit_once('/') {
        Some((d, n)) => (format!("{d}/"), n.to_string()),
        None => (String::new(), f.path.clone()),
    };
    let mut spans = vec![
        Span::styled(
            if folded { theme.icons.fold_closed.glyph } else { theme.icons.fold_open.glyph },
            muted,
        ),
        Span::styled(
            format!("{} ", crate::commit::sign(f.kind)),
            Style::new().fg(kind_color).add_modifier(Modifier::BOLD),
        ),
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
        vec![
            Span::styled(format!("+{added}"), Style::new().fg(c.git.added.into())),
            Span::raw(" "),
            Span::styled(format!("−{removed}"), Style::new().fg(c.git.deleted.into())),
        ]
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
