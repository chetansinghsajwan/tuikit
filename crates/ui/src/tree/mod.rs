//! Tree rows: how every tool draws a level of a tree (files, changed files, the files of a commit,
//! ref folders, symbols), so they look alike. A row is its indent guides, a fold mark or icon, its
//! label, and whatever the tool adds on either side.
//!
//! - A **dir** (a folder of files or refs) shows an open or closed folder icon: no arrow.
//! - A **group** (a section or a symbol with members) has no folder icon of its own, so it shows `▾`
//!   (open) or `▸` (folded), then its icon if it has one.
//! - A **leaf** is the guides, then what the tool puts there.

use crate::component::{Component, Context};
use crate::icons::Icon;
use crate::table::{Cell, Column, Row, Table};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// The indent guides: a thin line at each level.
pub fn guides(cx: &Context, depth: usize) -> Span<'static> {
    let theme = cx.theme;
    Span::styled(
        format!("{} ", theme.icons.guide.glyph).repeat(depth),
        Style::new().fg(theme.colors.muted.into()),
    )
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
    active_scope_by(depths.len(), |i| depths[i], cursor)
}

/// [`active_scope`] for a tree of `len` rows whose depths `depth` gives by row, so a table asks only for
/// the rows it walks.
pub fn active_scope_by(len: usize, depth: impl Fn(usize) -> usize, cursor: usize) -> Option<Scope> {
    if cursor >= len {
        return None;
    }
    let cur = depth(cursor);
    let folder = if cursor + 1 < len && depth(cursor + 1) > cur {
        cursor
    } else {
        (0..cursor).rev().find(|&i| depth(i) + 1 == cur)?
    };
    let at = depth(folder);
    let to = (folder + 1..len).find(|&i| depth(i) <= at).unwrap_or(len);
    Some(Scope { from: folder, to, col: at })
}

/// The indentation of every line with the blank ones (`None`) filled in: a blank line takes the smaller of its
/// neighbors' indentation, so guides run through it.
pub fn indent_levels(own: &[Option<usize>]) -> Vec<usize> {
    let mut before = 0;
    let before: Vec<usize> = own
        .iter()
        .map(|d| {
            if let Some(d) = d {
                before = *d;
            }
            before
        })
        .collect();
    let mut after = 0;
    let mut after_rev: Vec<usize> = own
        .iter()
        .rev()
        .map(|d| {
            if let Some(d) = d {
                after = *d;
            }
            after
        })
        .collect();
    after_rev.reverse();
    own.iter().enumerate().map(|(i, d)| d.unwrap_or_else(|| before[i].min(after_rev[i]))).collect()
}

/// The scope the cursor line is in, for code (an editor, a preview with a cursor): `indents` is each line's leading
/// whitespace in cells (`None` for a blank line) and `unit` the cells of one indent level. The same rule as in a tree:
/// the block the cursor line opens, else the block that holds it. The caller passes the lines and the cursor, nothing else.
pub fn code_scope(indents: &[Option<usize>], unit: usize, cursor: usize) -> Option<Scope> {
    let depths: Vec<usize> = indent_levels(indents).into_iter().map(|c| c / unit.max(1)).collect();
    active_scope(&depths, cursor)
}

/// The indent guides with the one at column `active` highlighted (the accent), like the editor marks the
/// scope the cursor is in. Without an active column it is just [`guides`].
pub fn guides_marked(cx: &Context, depth: usize, active: Option<usize>) -> Vec<Span<'static>> {
    let theme = cx.theme;
    let Some(col) = active.filter(|c| *c < depth) else { return vec![guides(cx, depth)] };
    let muted = Style::new().fg(theme.colors.muted.into());
    vec![
        Span::styled(format!("{} ", theme.icons.guide.glyph).repeat(col), muted),
        Span::styled(
            format!("{} ", theme.icons.guide.glyph),
            Style::new().fg(theme.colors.accent.into()),
        ),
        Span::styled(format!("{} ", theme.icons.guide.glyph).repeat(depth - col - 1), muted),
    ]
}

/// The start of a folder row: guides, the open or closed folder icon, and the name (accent, bold).
pub fn dir(cx: &Context, depth: usize, name: &str, open: bool) -> Vec<Span<'static>> {
    let theme = cx.theme;
    let icon = theme.icons.dir(name.rsplit('/').next().unwrap_or(name), open);
    vec![
        guides(cx, depth),
        Span::styled(format!("{} ", icon.glyph), icon.style()),
        Span::styled(
            name.to_string(),
            Style::new().fg(theme.colors.accent.into()).add_modifier(Modifier::BOLD),
        ),
    ]
}

/// The fold arrow of a row that can fold (`Some(true)` folded), or blank space of the same width for one that cannot.
pub fn fold_mark(cx: &Context, folded: Option<bool>) -> Span<'static> {
    let theme = cx.theme;
    let text = match folded {
        Some(true) => theme.icons.fold_closed.glyph,
        Some(false) => theme.icons.fold_open.glyph,
        None => "  ",
    };
    Span::styled(text, Style::new().fg(theme.colors.muted.into()))
}

/// The start of a group row: guides, the fold arrow, the icon if there is one, and the label in `style`.
pub fn group(
    cx: &Context,
    depth: usize,
    folded: bool,
    icon: Option<Icon>,
    label: &str,
    style: Style,
) -> Vec<Span<'static>> {
    let mut spans = vec![guides(cx, depth), fold_mark(cx, Some(folded))];
    if let Some(icon) = icon {
        spans.push(Span::styled(format!("{} ", icon.glyph), icon.style()));
    }
    spans.push(Span::styled(label.to_string(), style));
    spans
}

/// The count after a folder or group name: ` 3`.
pub fn count(cx: &Context, n: usize) -> Span<'static> {
    let theme = cx.theme;
    Span::styled(format!(" {n}"), Style::new().fg(theme.colors.muted.into()))
}

/// Draws indent guides into the spans of a line of code: a thin `│` (muted) in place of the space at every
/// `unit`-th column left of `indent`, the line's own (or, for a blank line, its neighbors') indentation. Only
/// leading spaces change; the rest of the line is untouched. A tool passes the content and the indentation; the
/// drawing is the toolkit's, so the guides of every code view look alike.
pub fn mark_indent(
    cx: &Context,
    spans: Vec<Span<'static>>,
    indent: usize,
    unit: usize,
) -> Vec<Span<'static>> {
    let theme = cx.theme;
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
                out.push(Span::styled(
                    theme.icons.guide.glyph,
                    guide.bg(span.style.bg.unwrap_or(Color::Reset)),
                ));
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
pub fn line_counts(
    cx: &Context,
    change: Option<(u32, u32)>,
    widths: CountWidths,
) -> Vec<Span<'static>> {
    let theme = cx.theme;
    let c = &theme.colors;
    let total = widths.width();
    match change {
        Some((a, r)) => {
            let used = widths.added + 1 + widths.removed;
            vec![
                Span::raw(" ".repeat(total - used)),
                Span::styled(
                    format!("{:>w$}", format!("+{a}"), w = widths.added),
                    Style::new().fg(c.git.added.into()),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:<w$}", format!("−{r}"), w = widths.removed),
                    Style::new().fg(c.git.deleted.into()),
                ),
            ]
        }
        None => {
            vec![Span::styled(format!("{:>total$}", "binary"), Style::new().fg(c.muted.into()))]
        }
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
    pub is_open: bool,
    /// A file is executable (its icon).
    pub is_executable: bool,
    /// An icon to use instead of the one the name and kind give (a submodule).
    pub icon: Option<Icon>,
    /// The git color of the row (or any status color): tints the row and colors the name.
    pub tint: Option<Color>,

    // State.
    pub is_selected: bool,
    /// The pane has the focus. A selected row of an unfocused pane is the faint "current" row.
    pub is_focused: bool,
    /// The indent guide to highlight, by column: the scope the cursor is in. Rows of one scope all pass the same column.
    pub active_guide: Option<usize>,
    /// Picked by the user: yellow and underlined.
    pub is_marked: bool,
    /// Cut to the clipboard: dimmed.
    pub is_cut: bool,
    /// A dot file: the name is muted.
    pub is_hidden: bool,
    /// Git ignores it (shown only when the tool asks for ignored files): muted, dimmed and italic, so it reads
    /// fainter than a dot file; a dot file that is also ignored gets both.
    pub is_ignored: bool,

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

/// The rows that enclose row `at` of a tree whose rows sit at `depths`, outermost first: for each
/// level above it, the nearest row before it that `is_container` (a folder). What a tree pins above
/// its first scrolling row (`TableView::pinned`; `TableView::tree` pins by depth alone).
pub fn ancestors(depths: &[usize], is_container: impl Fn(usize) -> bool, at: usize) -> Vec<usize> {
    let Some(&depth) = depths.get(at) else { return Vec::new() };
    let mut need = depth;
    let mut found = Vec::new();
    for i in (0..at).rev() {
        if need == 0 {
            break;
        }
        if is_container(i) && depths[i] < need {
            found.push(i);
            need = depths[i];
        }
    }
    found.reverse();
    found
}

/// The rows that enclose row `at` of a tree whose depths `depth` gives by row, outermost first: for each
/// level above it, the nearest row before it that is shallower (its parent, in a tree). What a `Table`
/// with `tree` pins above its first scrolling row.
pub fn ancestors_by(depth: impl Fn(usize) -> usize, at: usize) -> Vec<usize> {
    let mut need = depth(at);
    let mut found = Vec::new();
    for i in (0..at).rev() {
        if need == 0 {
            break;
        }
        let d = depth(i);
        if d < need {
            found.push(i);
            need = d;
        }
    }
    found.reverse();
    found
}

impl FileRow<'_> {
    /// The row for a `Table` drawn in [`file_columns`]: the icon, the name (git-colored, folders bold in
    /// the accent; the part that shrinks), the optional count and note, the trailing text and the signs.
    /// The table adds the indent guides (`TableView::tree`), the cursor and the focus.
    pub fn row(&self, cx: &Context) -> Row {
        Row {
            id: crate::anim::hash_of(&(self.depth, self.name)),
            // What the row shows apart from the cursor.
            content: crate::anim::hash_of(&(
                self.tint,
                self.is_dir,
                self.is_executable,
                self.is_marked,
                self.is_cut,
                self.is_hidden,
                self.is_ignored,
                self.count,
                self.note,
                self.signs.clone(),
                self.trailing.iter().map(|s| s.content.to_string()).collect::<Vec<_>>(),
            )),
            cells: vec![name_cell(cx, self), Cell::new(self.trailing.clone())],
            tint: self.tint,
            signs: self.signs.clone(),
            is_marked: false,
        }
    }
}

/// The columns a file row is drawn in: the name, the trailing text (sizes and times, or line counts),
/// muted at the right and dropped first when the name would get under four cells, and the signs.
pub fn file_columns() -> Vec<Column> {
    vec![Column::list(), Column::trailing(), Column::signs()]
}

/// The name cell of a file row: the icon, the name (the part that shrinks), the count and the note.
fn name_cell(cx: &Context, row: &FileRow) -> Cell {
    let theme = cx.theme;
    let c = &theme.colors;
    let mut name_style =
        Style::new().fg(row.tint.unwrap_or(if row.is_dir { c.accent.into() } else { c.fg.into() }));
    if row.is_dir {
        name_style = name_style.add_modifier(Modifier::BOLD);
    }
    if (row.is_hidden || row.is_ignored) && row.tint.is_none() {
        name_style = name_style.fg(c.muted.into());
    }
    if row.is_ignored {
        name_style = name_style.add_modifier(Modifier::DIM | Modifier::ITALIC);
    }
    if row.is_marked {
        name_style = name_style.fg(c.warning.into()).add_modifier(Modifier::UNDERLINED);
    }
    if row.is_cut {
        name_style = name_style.add_modifier(Modifier::DIM);
    }
    let base_name = row.name.rsplit('/').next().unwrap_or(row.name);
    let icon = row.icon.unwrap_or_else(|| {
        if row.is_dir {
            theme.icons.dir(base_name, row.is_open)
        } else {
            theme.icons.file(base_name, row.is_executable)
        }
    });
    let mut spans = vec![
        Span::styled(format!("{} ", icon.glyph), icon.style()),
        Span::styled(row.name.to_string(), name_style),
    ];
    if let Some(n) = row.count {
        spans.push(count(cx, n));
    }
    if let Some(note) = row.note {
        spans.push(Span::styled(format!("  {note}"), Style::new().fg(c.muted.into())));
    }
    Cell::new(spans).shrinking(1)
}

/// The same row as a [`Line`] of `width` cells, for places that build lines and not a buffer (a preview),
/// with its guides (`active_guide` highlighted). With no width only the name part is built: no trailing
/// text and no signs, as they need a right edge.
pub fn file_row_line(cx: &Context, row: &FileRow, width: Option<u16>) -> Line<'static> {
    let mut name = name_cell(cx, row);
    name.prepend(guides_marked(cx, row.depth, row.active_guide));
    let Some(w) = width else { return Line::from(name.spans) };
    let area = Rect::new(0, 0, w, 1);
    let mut buf = Buffer::empty(area);
    let mut built = row.row(cx);
    built.cells[0] = name;
    let columns = file_columns();
    Table::new()
        .view(1, &columns, |_| built.clone())
        .focused(row.is_focused)
        .cursor(row.is_selected)
        .render(&Context::still(cx.theme), area, &mut buf);
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
mod tests;

#[cfg(test)]
mod code_scope_tests;
