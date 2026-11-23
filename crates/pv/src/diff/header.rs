use git_core::Hunk;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ui::Theme;

/// The line that opens a hunk: where the change is, by name when there is one (the numbers are in the gutter below).
/// `before` and `after` are the lines of the file's old and new text, which name the function the hunk sits in.
pub(super) fn hunk_header(
    h: &Hunk,
    before: &[&str],
    after: &[&str],
    theme: &Theme,
) -> Line<'static> {
    let c = &theme.colors;
    let context = if !after.is_empty() {
        context_of(after, h.new_start.saturating_sub(1) as usize)
    } else {
        context_of(before, h.old_start.saturating_sub(1) as usize)
    };
    let mut header = vec![Span::styled(
        format!("  ⋯ line {}", h.new_start.max(1)),
        Style::new().fg(c.info.into()),
    )];
    if let Some(ctx) = context {
        header.push(Span::styled(
            format!("  {ctx}"),
            Style::new().fg(c.fg.into()).add_modifier(Modifier::BOLD),
        ));
    }
    Line::from(header)
}

/// The line a hunk sits in, like git's `@@ ... @@ fn name`: the nearest line above `from` (0-based) that starts a
/// function, a type, a section, or any line starting at column 0 with a letter, `_` or `$`.
fn context_of(lines: &[&str], from: usize) -> Option<String> {
    /// Line starts that open a function, type or section, for `context_of`.
    const KEYWORDS: [&str; 14] = [
        "fn ",
        "pub fn ",
        "async fn ",
        "def ",
        "class ",
        "func ",
        "function ",
        "impl ",
        "struct ",
        "enum ",
        "trait ",
        "interface ",
        "public ",
        "private ",
    ];
    let upto = from.min(lines.len());
    for line in lines[..upto].iter().rev() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let top = line
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$' || c == '#');
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
