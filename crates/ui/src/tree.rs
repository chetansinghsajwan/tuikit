//! Tree rows: how every tool draws a level of a tree (files, changed files, the files of a commit,
//! ref folders, symbols), so they look alike. A row is its indent guides, a fold mark or icon, its
//! label, and whatever the tool adds on either side.
//!
//! - A **dir** (a folder of files or refs) shows an open or closed folder icon: no arrow.
//! - A **group** (a section or a symbol with members) has no folder icon of its own, so it shows `▾`
//!   (open) or `▸` (folded), then its icon if it has one.
//! - A **leaf** is the guides, then what the tool puts there.

use crate::icons::Icon;
use crate::list;
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

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

/// The count after a folder or group name: ` 3`.
pub fn count(theme: &Theme, n: usize) -> Span<'static> {
    Span::styled(format!(" {n}"), Style::new().fg(theme.colors.muted.into()))
}

/// What a tool passes to [`file_row`]: the content of one row of a file tree and which optional
/// parts it wants. The structure and the drawing are the toolkit's; a tool never draws this row
/// itself. A part left at its default is simply not shown.
#[derive(Default)]
pub struct FileRow<'a> {
    // Content.
    pub depth: usize,
    pub name: &'a str,
    pub is_dir: bool,
    /// A folder is open (its icon).
    pub open: bool,
    /// A file is executable (its icon).
    pub executable: bool,
    /// The git color of the row (or any status color): tints the row and colors the name.
    pub tint: Option<Color>,

    // State.
    pub selected: bool,
    /// The pane has the focus. A selected row of an unfocused pane is the faint "current" row.
    pub focused: bool,
    /// Picked by the user: yellow and underlined.
    pub marked: bool,
    /// Cut to the clipboard: dimmed.
    pub cut: bool,

    // Options, all off by default.
    /// The number of entries beneath a folder, after its name.
    pub count: Option<usize>,
    /// Muted-style text right-aligned at the end of the row, before the signs: sizes and times, or the `+n -n`
    /// line counts of a change. The caller decides whether there is room. Give only a foreground; the row
    /// supplies the background.
    pub trailing: Vec<Span<'static>>,
    /// Signs at the right edge, the most serious first (git letters). The last cell of every row is kept for
    /// them, so the trailing text lines up whether a row has a sign or not.
    pub signs: Vec<(char, Color)>,
}

/// Draws one row of a file tree at line `y`: the bar and highlight, the indent guides, the icon, the name
/// (git-colored, folders bold in the accent), then the optional count, trailing text and signs.
pub fn file_row(theme: &Theme, buf: &mut Buffer, rows: &list::Rows, y: u16, row: &FileRow) {
    let c = &theme.colors;
    let base = if row.selected && !row.focused { list::active(theme) } else { list::style(theme, row.selected && row.focused, row.tint) };
    buf.set_style(rows.bar(y), base);
    let area = rows.content(y);

    let mut name_style = Style::new().fg(row.tint.unwrap_or(if row.is_dir { c.accent.into() } else { c.fg.into() }));
    if row.is_dir {
        name_style = name_style.add_modifier(Modifier::BOLD);
    }
    if row.marked {
        name_style = name_style.fg(c.warning.into()).add_modifier(Modifier::UNDERLINED);
    }
    if row.cut {
        name_style = name_style.add_modifier(Modifier::DIM);
    }
    let base_name = row.name.rsplit('/').next().unwrap_or(row.name);
    let icon = if row.is_dir { crate::icons::dir(base_name, row.open) } else { crate::icons::file(base_name, row.executable) };
    let mut spans = vec![guides(theme, row.depth), Span::styled(format!("{} ", icon.glyph), Style::new().fg(icon.color)), Span::styled(row.name.to_string(), name_style)];
    if let Some(n) = row.count {
        spans.push(count(theme, n));
    }

    let trailing: u16 = row.trailing.iter().map(|s| s.content.width() as u16).sum();
    Paragraph::new(Line::from(spans)).style(base).render(Rect { width: area.width.saturating_sub(trailing), ..area }, buf);
    if trailing > 0 {
        let x = area.right().saturating_sub(trailing + 1);
        Paragraph::new(Line::from(row.trailing.clone())).style(base).render(Rect { x, width: trailing, ..area }, buf);
    }

    let mut x = area.right().saturating_sub(1);
    for (i, (ch, color)) in row.signs.iter().enumerate().rev() {
        buf.set_string(x, y, ch.to_string(), base.fg(*color));
        x = x.saturating_sub(if i == 0 { 0 } else { 2 });
    }
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
        assert_eq!(text(&[count(&theme, 3)]), " 3");
    }

    fn row_text(row: &FileRow, width: u16) -> String {
        let theme = Theme::load();
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        file_row(&theme, &mut buf, &list::Rows::new(area, area), 0, row);
        (0..width).map(|x| buf[(x, 0)].symbol().to_string()).collect()
    }

    #[test]
    fn a_file_row_shows_only_the_options_asked_for() {
        let plain = FileRow { name: "main.rs", ..Default::default() };
        let t = row_text(&plain, 40);
        assert!(t.contains("main.rs") && !t.contains('+') && t.trim_end().ends_with("main.rs"), "{t}");

        let dir = FileRow { name: "src", is_dir: true, count: Some(4), ..Default::default() };
        assert!(row_text(&dir, 40).contains("src 4"));

        let full = FileRow {
            name: "main.rs",
            trailing: vec![Span::raw("+3 -1")],
            signs: vec![('M', Color::Red)],
            ..Default::default()
        };
        let t = row_text(&full, 40);
        assert!(t.trim_end().ends_with("+3 -1M"), "trailing text ends just left of the sign cell: {t}");
    }
}
