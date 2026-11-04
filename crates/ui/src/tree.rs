//! Tree rows: how every tool draws a level of a tree (files, changed files, the files of a commit,
//! ref folders, symbols), so they look alike. A row is its indent guides, a fold mark or icon, its
//! label, and whatever the tool adds on either side.
//!
//! - A **dir** (a folder of files or refs) shows an open or closed folder icon: no arrow.
//! - A **group** (a section or a symbol with members) has no folder icon of its own, so it shows `▾`
//!   (open) or `▸` (folded), then its icon if it has one.
//! - A **leaf** is the guides, then what the tool puts there.

use crate::icons::Icon;
use crate::theme::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

/// The indent guides: a thin line at each level.
pub fn guides(theme: &Theme, depth: usize) -> Span<'static> {
    Span::styled("│ ".repeat(depth), Style::new().fg(theme.colors.muted.into()))
}

/// The start of a folder row: guides, the open or closed folder icon, and the name (accent, bold).
pub fn dir(theme: &Theme, depth: usize, name: &str, open: bool) -> Vec<Span<'static>> {
    let icon = crate::icons::dir(name.rsplit('/').next().unwrap_or(name), open);
    vec![guides(theme, depth), Span::styled(format!("{} ", icon.glyph), Style::new().fg(icon.color)), Span::styled(name.to_string(), Style::new().fg(theme.colors.accent.into()).add_modifier(Modifier::BOLD))]
}

/// The fold arrow of a row that can fold (`Some(true)` folded), or blank space of the same width for one that cannot.
pub fn fold_mark(theme: &Theme, folded: Option<bool>) -> Span<'static> {
    let text = match folded {
        Some(true) => "▸ ",
        Some(false) => "▾ ",
        None => "  ",
    };
    Span::styled(text, Style::new().fg(theme.colors.muted.into()))
}

/// The start of a group row: guides, the fold arrow, the icon if there is one, and the label in `style`.
pub fn group(theme: &Theme, depth: usize, folded: bool, icon: Option<Icon>, label: &str, style: Style) -> Vec<Span<'static>> {
    let mut spans = vec![guides(theme, depth), fold_mark(theme, Some(folded))];
    if let Some(icon) = icon {
        spans.push(Span::styled(format!("{} ", icon.glyph), Style::new().fg(icon.color)));
    }
    spans.push(Span::styled(label.to_string(), style));
    spans
}

/// The count after a folder or group name: `(3)`.
pub fn count(theme: &Theme, n: usize) -> Span<'static> {
    Span::styled(format!(" ({n})"), Style::new().fg(theme.colors.muted.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn a_dir_has_guides_and_a_folder_icon_a_group_has_an_arrow() {
        let theme = Theme::load();
        let d = text(&dir(&theme, 2, "src", true));
        assert!(d.starts_with("│ │ ") && d.ends_with("src") && !d.contains('▾'), "{d}");
        assert!(text(&group(&theme, 1, true, None, "Tags", Style::new())).starts_with("│ ▸ Tags"));
        assert!(text(&group(&theme, 0, false, None, "Tags", Style::new())).starts_with("▾ Tags"));
        assert_eq!(text(&[count(&theme, 3)]), " (3)");
    }
}
