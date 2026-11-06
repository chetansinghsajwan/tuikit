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

/// The scope the cursor is in, for the highlighted indent guide: the rows `from < row < to` share the guide at
/// column `col`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scope {
    pub from: usize,
    pub to: usize,
    pub col: usize,
}

impl Scope {
    /// The guide column to highlight on `row` (pass it as [`FileRow::active_guide`] or to [`guides_marked`]).
    pub fn col_at(&self, row: usize) -> Option<usize> {
        (row > self.from && row < self.to).then_some(self.col)
    }
}

/// The scope of the row under the cursor, from the depth of every row of a tree: the folder it is (it has
/// deeper rows right below), else the folder that holds it. A row at the top with nothing below has none.
/// Every tree view calls this, so the highlight needs no option and behaves the same everywhere.
pub fn active_scope(depths: &[usize], cursor: usize) -> Option<Scope> {
    let cur = *depths.get(cursor)?;
    let folder = if depths.get(cursor + 1).is_some_and(|d| *d > cur) { cursor } else { depths[..cursor].iter().rposition(|d| *d + 1 == cur)? };
    let depth = depths[folder];
    let to = depths[folder + 1..].iter().position(|d| *d <= depth).map_or(depths.len(), |n| folder + 1 + n);
    Some(Scope { from: folder, to, col: depth })
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

/// Draws indent guides into the spans of a line of code: a thin `│` (muted) in place of the space at every
/// `unit`-th column left of `indent`, the line's own (or, for a blank line, its neighbors') indentation. Only
/// leading spaces change; the rest of the line is untouched. A tool passes the content and the indentation; the
/// drawing is the toolkit's, so the guides of every code view look alike.
pub fn mark_indent(theme: &Theme, spans: Vec<Span<'static>>, indent: usize, unit: usize) -> Vec<Span<'static>> {
    let guide = Style::new().fg(theme.colors.muted.into()).add_modifier(Modifier::DIM);
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut col = 0;
    for span in spans {
        if col >= indent || unit == 0 {
            out.push(span);
            continue;
        }
        let mut plain = String::new();
        let mut rest = String::new();
        let mut chars = span.content.chars();
        for ch in chars.by_ref() {
            if col >= indent || ch != ' ' {
                // Past the indentation (or the first non-space): the rest of this span stays as it is.
                rest.push(ch);
                rest.extend(chars.by_ref());
                break;
            }
            if col % unit == 0 {
                if !plain.is_empty() {
                    out.push(Span::styled(std::mem::take(&mut plain), span.style));
                }
                out.push(Span::styled("│", guide.bg(span.style.bg.unwrap_or(Color::Reset))));
            } else {
                plain.push(ch);
            }
            col += 1;
        }
        plain.push_str(&rest);
        if !plain.is_empty() {
            out.push(Span::styled(plain, span.style));
        }
        // A character that stopped the leading run ends the indentation for the following spans too.
        if !rest.is_empty() {
            col = indent;
        }
    }
    out
}

/// The widths of the line-count columns (`+12 −3`) of a list of changes. Every row of the list is laid out with
/// the same ones, so the `+`s line up and so do the `−`s. The tool gives the numbers; the layout is the toolkit's.
#[derive(Debug, Clone, Copy, Default)]
pub struct CountWidths {
    added: usize,
    removed: usize,
    binary: bool,
}

impl CountWidths {
    /// From the `(added, removed)` of every row of the list, `None` for a binary file.
    pub fn of(changes: impl IntoIterator<Item = Option<(u32, u32)>>) -> CountWidths {
        let mut w = CountWidths { added: 2, removed: 2, binary: false };
        for c in changes {
            match c {
                Some((a, r)) => {
                    w.added = w.added.max(a.to_string().len() + 1);
                    w.removed = w.removed.max(r.to_string().len() + 1);
                }
                None => w.binary = true,
            }
        }
        w
    }

    /// Cells one row of counts takes.
    pub fn width(&self) -> usize {
        (self.added + 1 + self.removed).max(if self.binary { 6 } else { 0 })
    }
}

/// One row's line counts in the columns of `widths`: `+12` right-aligned, `−3` left-aligned, in the git
/// colors; `None` (a binary file) reads `binary`, muted. Always `widths.width()` cells.
pub fn line_counts(theme: &Theme, change: Option<(u32, u32)>, widths: CountWidths) -> Vec<Span<'static>> {
    let c = &theme.colors;
    let total = widths.width();
    match change {
        Some((a, r)) => {
            let used = widths.added + 1 + widths.removed;
            vec![
                Span::raw(" ".repeat(total - used)),
                Span::styled(format!("{:>w$}", format!("+{a}"), w = widths.added), Style::new().fg(c.git.added.into())),
                Span::raw(" "),
                Span::styled(format!("{:<w$}", format!("−{r}"), w = widths.removed), Style::new().fg(c.git.deleted.into())),
            ]
        }
        None => vec![Span::styled(format!("{:>total$}", "binary"), Style::new().fg(c.muted.into()))],
    }
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
    draw_row(theme, buf, rows, y, row);
    // Noted for the motion (`anim`): the row is identified by its place in the tree, its content leaves out the cursor.
    let content = crate::anim::hash_of(&(row.tint, row.is_dir, row.executable, row.marked, row.cut, row.count, row.note, row.signs.clone(), row.trailing.iter().map(|s| s.content.to_string()).collect::<Vec<_>>()));
    crate::anim::note_row(rows.id(), y, crate::anim::hash_of(&(row.depth, row.name)), content, rows.bar(y));
}

fn draw_row(theme: &Theme, buf: &mut Buffer, rows: &list::Rows, y: u16, row: &FileRow) {
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
    let signs = if row.signs.is_empty() { 1 } else { row.signs.len() * 2 };
    let count_w = row.count.map_or(0, |n| format!(" {n}").width()) + row.note.map_or(0, |n| n.width() + 2);
    let budget = |trailing: usize| width.saturating_sub(signs.max(1 + trailing + usize::from(trailing > 0))).saturating_sub(row.depth * 2 + 2 + count_w);
    let mut trailing: usize = row.trailing.iter().map(|s| s.content.width()).sum();
    let show_trailing = trailing > 0 && budget(trailing) >= MIN_NAME;
    if !show_trailing {
        trailing = 0;
    }
    let limit = width.saturating_sub(signs.max(1 + trailing + usize::from(trailing > 0)));
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
    draw_row(theme, &mut buf, &list::Rows::new(area, area), 0, row);
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
    fn the_scope_is_the_folder_under_the_cursor_or_the_one_holding_it() {
        // a / a/b / a/b/x / a/y / z
        let depths = [0, 1, 2, 1, 0];
        assert_eq!(active_scope(&depths, 1), Some(Scope { from: 1, to: 3, col: 1 }), "an open folder: its contents");
        assert_eq!(active_scope(&depths, 2), Some(Scope { from: 1, to: 3, col: 1 }), "a file: the folder that holds it");
        assert_eq!(active_scope(&depths, 3), Some(Scope { from: 0, to: 4, col: 0 }));
        assert_eq!(active_scope(&depths, 4), None, "a top-level file has no scope");
        assert_eq!(active_scope(&depths, 0).and_then(|s| s.col_at(2)), Some(0));
        assert_eq!(active_scope(&depths, 0).and_then(|s| s.col_at(4)), None);
    }

    #[test]
    fn indent_guides_replace_the_leading_spaces_at_each_unit() {
        let theme = Theme::load();
        let text = |s: &[Span]| -> String { s.iter().map(|s| s.content.as_ref()).collect() };
        let line = vec![Span::raw("        "), Span::raw("let x = 1;")];
        assert_eq!(text(&mark_indent(&theme, line, 8, 4)), "│   │   let x = 1;");
        // Only up to the indentation given (a blank line borrows its neighbors'), and never inside the code.
        assert_eq!(text(&mark_indent(&theme, vec![Span::raw("      a  b")], 4, 2)), "│ │   a  b");
        assert_eq!(text(&mark_indent(&theme, vec![Span::raw("    ")], 0, 4)), "    ");
        // The spaces of one span split across highlight pieces still count as one column run.
        assert_eq!(text(&mark_indent(&theme, vec![Span::raw("  "), Span::raw("  "), Span::raw("x")], 4, 2)), "│ │ x");
    }

    #[test]
    fn line_counts_line_up_in_columns() {
        let theme = Theme::load();
        let text = |s: &[Span]| -> String { s.iter().map(|s| s.content.as_ref()).collect() };
        let changes = [Some((3, 120)), Some((1204, 4)), None];
        let w = CountWidths::of(changes);
        let rows: Vec<String> = changes.iter().map(|c| text(&line_counts(&theme, *c, w))).collect();
        assert!(rows.iter().all(|r| r.chars().count() == w.width()), "{rows:?}");
        assert_eq!(rows[0], "   +3 −120", "{rows:?}");
        assert_eq!(rows[1], "+1204 −4  ", "{rows:?}");
        assert_eq!(rows[2], "    binary", "{rows:?}");
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
