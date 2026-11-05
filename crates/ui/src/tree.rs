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

/// The indent guides with the one at column `active` highlighted (the accent), like the editor marks the
/// scope the cursor is in. Without an active column it is just [`guides`].
pub fn guides_marked(theme: &Theme, depth: usize, active: Option<usize>) -> Vec<Span<'static>> {
    let Some(col) = active.filter(|c| *c < depth) else { return vec![guides(theme, depth)] };
    let muted = Style::new().fg(theme.colors.muted.into());
    vec![Span::styled("│ ".repeat(col), muted), Span::styled("│ ", Style::new().fg(theme.colors.accent.into())), Span::styled("│ ".repeat(depth - col - 1), muted)]
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
    /// The indent guide to highlight, by column: the scope the cursor is in. Rows of one scope all pass the same column.
    pub active_guide: Option<usize>,
    /// Picked by the user: yellow and underlined.
    pub marked: bool,
    /// Cut to the clipboard: dimmed.
    pub cut: bool,

    // Options, all off by default.
    /// The number of entries beneath a folder, after its name.
    pub count: Option<usize>,
    /// A muted remark after the name (and the count): `(empty)`.
    pub note: Option<&'a str>,
    /// Muted-style text right-aligned at the end of the row, before the signs: sizes and times, or the `+n -n`
    /// line counts of a change. The caller decides whether there is room. Give only a foreground; the row
    /// supplies the background.
    pub trailing: Vec<Span<'static>>,
    /// Signs at the right edge, the most serious first (git letters). The last cell of every row is kept for
    /// them, so the trailing text lines up whether a row has a sign or not.
    pub signs: Vec<(char, Color)>,
}

/// The fewest cells of a name worth showing beside the trailing text.
const MIN_NAME: usize = 4;

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
    let mut spans = guides_marked(theme, row.depth, row.active_guide);
    spans.push(Span::styled(format!("{} ", icon.glyph), Style::new().fg(icon.color)));
    let name_at = spans.len();
    spans.push(Span::raw(""));
    if let Some(n) = row.count {
        spans.push(count(theme, n));
    }
    if let Some(note) = row.note {
        spans.push(Span::styled(format!("  {note}"), Style::new().fg(c.muted.into())));
    }

    // Room for the name: what is left of the row after the guides, the icon, the count, the trailing text and
    // the signs. The name shrinks (`…`) to fit; trailing text that would leave it under `MIN_NAME` cells
    // is dropped, so a narrow pane keeps the name.
    let width = usize::from(area.width);
    let signs = row.signs.len().saturating_mul(2).saturating_sub(1).max(1);
    let count_w = row.count.map_or(0, |n| format!(" {n}").width()) + row.note.map_or(0, |n| n.width() + 2);
    let budget = |trailing: usize| width.saturating_sub(signs.max(1 + trailing)).saturating_sub(row.depth * 2 + 2 + count_w);
    let mut trailing: usize = row.trailing.iter().map(|s| s.content.width()).sum();
    let show_trailing = trailing > 0 && budget(trailing) >= MIN_NAME;
    if !show_trailing {
        trailing = 0;
    }
    let limit = width.saturating_sub(signs.max(1 + trailing));
    spans[name_at] = Span::styled(crate::list::fit(row.name, budget(trailing)), name_style);
    let trailing = trailing as u16;
    Paragraph::new(Line::from(spans)).style(base).render(Rect { width: limit as u16, ..area }, buf);
    if show_trailing {
        let x = area.right().saturating_sub(trailing + 1);
        Paragraph::new(Line::from(row.trailing.clone())).style(base).render(Rect { x, width: trailing, ..area }, buf);
    }

    let mut x = area.right().saturating_sub(1);
    for (i, (ch, color)) in row.signs.iter().enumerate().rev() {
        buf.set_string(x, y, ch.to_string(), base.fg(*color));
        x = x.saturating_sub(if i == 0 { 0 } else { 2 });
    }
}

/// The same row as a [`Line`] of `width` cells, for places that build lines and not a buffer (a preview). With
/// no width only the name part is built: no trailing text and no signs, as they need a right edge.
pub fn file_row_line(theme: &Theme, row: &FileRow, width: Option<u16>) -> Line<'static> {
    let Some(w) = width else {
        let mut spans = guides_marked(theme, row.depth, row.active_guide);
        let base_name = row.name.rsplit('/').next().unwrap_or(row.name);
        let icon = if row.is_dir { crate::icons::dir(base_name, row.open) } else { crate::icons::file(base_name, row.executable) };
        spans.push(Span::styled(format!("{} ", icon.glyph), Style::new().fg(icon.color)));
        let c = &theme.colors;
        let style = if row.is_dir { Style::new().fg(row.tint.unwrap_or(c.accent.into())).add_modifier(Modifier::BOLD) } else { Style::new().fg(row.tint.unwrap_or(c.fg.into())) };
        spans.push(Span::styled(row.name.to_string(), style));
        if let Some(n) = row.count {
            spans.push(count(theme, n));
        }
        if let Some(note) = row.note {
            spans.push(Span::styled(format!("  {note}"), Style::new().fg(c.muted.into())));
        }
        return Line::from(spans);
    };
    let area = Rect::new(0, 0, w, 1);
    let mut buf = Buffer::empty(area);
    file_row(theme, &mut buf, &list::Rows::new(area, area), 0, row);
    let mut runs: Vec<(Style, String)> = Vec::new();
    for x in 0..w {
        let cell = &buf[(x, 0)];
        match runs.last_mut() {
            Some((style, text)) if *style == cell.style() => text.push_str(cell.symbol()),
            _ => runs.push((cell.style(), cell.symbol().to_string())),
        }
    }
    Line::from(runs.into_iter().map(|(style, text)| Span::styled(text, style)).collect::<Vec<_>>())
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
    fn a_long_name_shrinks_and_the_rest_of_the_row_keeps_its_place() {
        let long = FileRow { name: "a-very-long-file-name-that-does-not-fit.rs", count: Some(7), trailing: vec![Span::raw("+3 -1 ")], signs: vec![('M', Color::Red)], ..Default::default() };
        let t = row_text(&long, 30);
        assert_eq!(t.chars().count(), 30);
        assert!(t.contains('…') && t.trim_end().ends_with("+3 -1 M"), "{t}");
        // A pane too narrow for the trailing text drops it and keeps the name.
        let t = row_text(&FileRow { trailing: vec![Span::raw("a-very-wide-column")], ..FileRow { name: "main.rs", ..Default::default() } }, 20);
        assert!(t.contains("main.rs") && !t.contains("column"), "{t}");
        // Wide characters are counted by cells, so the sign stays in the last cell.
        let t = row_text(&FileRow { name: "日本語日本語日本語日本語", signs: vec![('M', Color::Red)], ..Default::default() }, 16);
        assert!(t.trim_end().ends_with('M') && t.contains('…'), "{t}");
    }

    #[test]
    fn the_active_guide_is_drawn_in_the_accent() {
        let theme = Theme::load();
        let text = |s: &[Span]| -> String { s.iter().map(|s| s.content.as_ref()).collect() };
        let marked = guides_marked(&theme, 3, Some(1));
        assert_eq!(text(&marked), guides(&theme, 3).content);
        let accent: Color = theme.colors.accent.into();
        assert_eq!(marked.iter().filter(|s| s.style.fg == Some(accent)).map(|s| s.content.as_ref()).collect::<String>(), "│ ");
        // A column that is not among the guides marks nothing.
        assert_eq!(guides_marked(&theme, 1, Some(1)).len(), 1);
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
