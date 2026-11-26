//! Rows: what a tool gives for each visible row (its cells and the parts the table draws for it), and
//! how the table paints one into its columns.

use super::layout::{Align, Column, Kind, Placed};
use crate::component::Context;
use crate::list::{active, clip_spans, fit, style};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

/// The content of one cell: styled spans, and which of them gives way when the cell is too narrow (a
/// name, so the icon and the count beside it keep their place). Without one, the end gives way.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cell {
    pub spans: Vec<Span<'static>>,
    pub shrink: Option<usize>,
    /// The side of the cell the text sits on, instead of its column's (a number in a text column).
    pub align: Option<Align>,
}

impl Cell {
    pub fn new(spans: Vec<Span<'static>>) -> Cell {
        Cell { spans, shrink: None, align: None }
    }

    /// The text sits at the right of the cell, whatever its column says (a number).
    pub fn right(mut self) -> Cell {
        self.align = Some(Align::Right);
        self
    }

    /// The span at `index` is the one that shrinks (with `…`) when the cell is too narrow.
    pub fn shrinking(mut self, index: usize) -> Cell {
        self.shrink = Some(index);
        self
    }

    /// Its width in cells.
    pub fn width(&self) -> usize {
        self.spans.iter().map(|s| s.content.width()).sum()
    }

    /// The width of the part that does not shrink: all but the shrinking span (nothing when the end
    /// gives way).
    pub(crate) fn fixed_width(&self) -> usize {
        match self.shrink.and_then(|i| self.spans.get(i)) {
            Some(s) => self.width() - s.content.width(),
            None => 0,
        }
    }

    /// `spans` put in front (tree guides), keeping the same span the one that shrinks.
    pub(crate) fn prepend(&mut self, spans: Vec<Span<'static>>) {
        self.shrink = self.shrink.map(|i| i + spans.len());
        self.spans.splice(0..0, spans);
    }

    /// The text without styles, for measuring and for a row's default identity.
    pub(crate) fn text(&self) -> String {
        self.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    /// The spans cut to `width` cells: the shrinking span gives way first, then the end.
    pub(crate) fn fit(mut self, width: usize) -> Vec<Span<'static>> {
        let total = self.width();
        if total <= width {
            return self.spans;
        }
        if let Some(i) = self.shrink.filter(|&i| i < self.spans.len()) {
            let own = self.spans[i].content.width();
            let room = width.saturating_sub(total - own);
            if room > 0 {
                let s = &self.spans[i];
                self.spans[i] = Span::styled(fit(&s.content, room), s.style);
                return self.spans;
            }
        }
        clip_spans(self.spans, width)
    }
}

impl From<Vec<Span<'static>>> for Cell {
    fn from(spans: Vec<Span<'static>>) -> Cell {
        Cell::new(spans)
    }
}

impl From<Span<'static>> for Cell {
    fn from(span: Span<'static>) -> Cell {
        Cell::new(vec![span])
    }
}

impl From<String> for Cell {
    fn from(text: String) -> Cell {
        Cell::new(vec![Span::raw(text)])
    }
}

impl From<&'static str> for Cell {
    fn from(text: &'static str) -> Cell {
        Cell::new(vec![Span::raw(text)])
    }
}

/// One row of a table, as the tool builds it for the index the table asks for: its cells (one per
/// column of cells, in order; a missing one is blank) and what the table draws for it. The bar, the
/// cursor, the focus, the alignment and the cuts are the table's.
#[derive(Debug, Clone, Default)]
pub struct Row {
    /// What identifies the row across frames (its path, hash, name) and what it shows apart from the
    /// cursor, for the motion. Leave both 0 and the text of the cells stands in.
    pub id: u64,
    pub content: u64,
    pub cells: Vec<Cell>,
    /// A status color (git): the row's bar takes a faint tint of it.
    pub tint: Option<Color>,
    /// Signs for `Column::signs`, the most serious first (git letters).
    pub signs: Vec<(char, Color)>,
    /// Picked by the user, drawn in `Column::mark`.
    pub is_marked: bool,
}

impl Row {
    /// A row of these cells.
    pub fn new(cells: Vec<Cell>) -> Row {
        Row { cells, ..Row::default() }
    }

    /// The content each column shows for this row, in column order (`None` for a cell the row lacks).
    pub(crate) fn cells_for(&self, cx: &Context, columns: &[Column]) -> Vec<Option<Cell>> {
        let mut own = self.cells.iter();
        columns
            .iter()
            .map(|c| match c.kind {
                Kind::Cells => own.next().cloned(),
                Kind::Signs => Some(signs_cell(&self.signs)),
                Kind::Mark => Some(mark_cell(cx, self.is_marked)),
            })
            .collect()
    }

    /// The identity and content for the motion: the tool's, or the text of the cells.
    pub(crate) fn identity(&self) -> (u64, u64) {
        if self.id == 0 && self.content == 0 {
            let text: Vec<String> = self.cells.iter().map(Cell::text).collect();
            let first = text.first().cloned().unwrap_or_default();
            (crate::anim::hash_of(&first), crate::anim::hash_of(&text))
        } else {
            (self.id, self.content)
        }
    }
}

/// The signs as a cell: one cell each with a blank between, the most serious first.
fn signs_cell(signs: &[(char, Color)]) -> Cell {
    let mut spans = Vec::new();
    for (i, (ch, color)) in signs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(ch.to_string(), Style::new().fg(*color)));
    }
    Cell::new(spans)
}

/// The mark of a marked row, in the accent; blank otherwise.
fn mark_cell(cx: &Context, marked: bool) -> Cell {
    let theme = cx.theme;
    if marked {
        Cell::from(Span::styled(
            theme.icons.mark.glyph,
            Style::new().fg(theme.colors.accent.into()),
        ))
    } else {
        Cell::default()
    }
}

/// The geometry of one table: where its row bars and row contents go, so every tool lines up alike.
/// A row's *bar* (its background) spans the pane's padding on both sides, so the highlight touches the
/// borders; its *content* stays inside the padding, giving the text a one-cell margin within the bar.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Rows {
    inner: Rect,
    left: u16,
    right: u16,
}

impl Rows {
    /// `pane` is the whole pane, `inner` the table's area inside its border and padding.
    pub(crate) fn new(pane: Rect, inner: Rect) -> Rows {
        let left = inner.x.saturating_sub(pane.x + 1).min(1);
        let right = pane.right().saturating_sub(inner.right() + 1).min(1);
        Rows { inner, left, right }
    }

    /// Which table this is, for the motion: where its content starts.
    pub(crate) fn id(&self) -> (u16, u16) {
        (self.inner.x, self.inner.y)
    }

    /// The row's background at `y`, reaching into the padding on both sides.
    pub(crate) fn bar(&self, y: u16) -> Rect {
        Rect {
            x: self.inner.x - self.left,
            y,
            width: self.inner.width + self.left + self.right,
            height: 1,
        }
    }

    /// The row's content area at `y`, inside the padding.
    pub(crate) fn content(&self, y: u16) -> Rect {
        Rect { y, height: 1, ..self.inner }
    }
}

/// How the cursor and the focus show on a row.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct State {
    pub is_selected: bool,
    pub is_focused: bool,
    /// The cursor's column in cell mode: that cell is the bright one and the rest of the row the faint one.
    pub cell: Option<usize>,
}

/// The bar style of a row: the hovered one bright, the current row of an unfocused pane faint, a tinted
/// row tinted.
pub(crate) fn bar_style(cx: &Context, tint: Option<Color>, state: State) -> Style {
    let theme = cx.theme;
    if state.is_selected && (!state.is_focused || state.cell.is_some()) {
        active(theme)
    } else {
        style(theme, state.is_selected && state.is_focused, tint)
    }
}

/// The columns of one frame and where each is placed (`None`: not drawn), and whether a `│` is drawn
/// between them.
pub(crate) struct Placement<'a> {
    pub columns: &'a [Column],
    pub placed: &'a [Option<Placed>],
    pub separator: bool,
}

impl Placement<'_> {
    /// Draws the `│` between the columns drawn on line `y` of a row starting at `x`.
    fn separators(&self, cx: &Context, buf: &mut Buffer, x: u16, y: u16, base: Style) {
        if !self.separator {
            return;
        }
        let style = base.fg(cx.theme.colors.muted.into());
        let glyph = cx.theme.icons.guide.glyph;
        for at in self.placed.iter().flatten().skip(1) {
            buf.set_string(x + at.x - 2, y, glyph, style);
        }
    }
}

/// Paints `cells` (the row's content per column) on line `y` of `rows`, each column where `at` places
/// it, over the row's bar (`base`).
pub(crate) fn paint(
    cx: &Context,
    buf: &mut Buffer,
    (rows, y): (&Rows, u16),
    at: &Placement,
    cells: Vec<Option<Cell>>,
    base: Style,
    state: State,
) {
    let place = at;
    let (columns, placed) = (at.columns, at.placed);
    let theme = cx.theme;
    let muted = Style::new().fg(theme.colors.muted.into());
    buf.set_style(rows.bar(y), base);
    let area = rows.content(y);
    for (i, cell) in cells.into_iter().enumerate() {
        let (Some(at), Some(cell)) = (placed[i], cell) else { continue };
        let column = &columns[i];
        let width = usize::from(at.width);
        let align = cell.align.unwrap_or(column.align);
        let mut spans = cell.fit(width);
        if column.muted {
            spans =
                spans.into_iter().map(|s| Span::styled(s.content, muted.patch(s.style))).collect();
        }
        let used: usize = spans.iter().map(|s| s.content.width()).sum();
        let pad = width.saturating_sub(used) as u16;
        let x = area.x + at.x + if align == Align::Right { pad } else { 0 };
        let mut cell_base = base;
        if state.is_selected && state.cell == Some(i) {
            cell_base = style(theme, state.is_focused, None);
            buf.set_style(Rect { x: area.x + at.x, y, width: at.width, height: 1 }, cell_base);
        }
        Paragraph::new(Line::from(spans))
            .style(cell_base)
            .render(Rect { x, y, width: at.width - pad, height: 1 }, buf);
    }
    place.separators(cx, buf, area.x, y, base);
}

/// The header: each column's title where its cells go, muted, the sort column accent and bold with `▼`
/// (largest first) or `▲`.
pub(crate) fn header(
    cx: &Context,
    buf: &mut Buffer,
    area: Rect,
    place: &Placement,
    sort: Option<super::Sort>,
) {
    let (columns, placed) = (place.columns, place.placed);
    let theme = cx.theme;
    for (i, column) in columns.iter().enumerate() {
        let Some(at) = placed[i] else { continue };
        let span = match sort {
            Some(s) if s.column == column.id => Span::styled(
                format!("{}{}", column.title, if s.descending { '▼' } else { '▲' }),
                Style::new().fg(theme.colors.accent.into()).add_modifier(Modifier::BOLD),
            ),
            _ => Span::styled(column.title.to_string(), Style::new().fg(theme.colors.muted.into())),
        };
        let text = Cell::from(span).fit(usize::from(at.width));
        let used: usize = text.iter().map(|s| s.content.width()).sum();
        let pad = usize::from(at.width).saturating_sub(used) as u16;
        let x = area.x + at.x + if column.align == Align::Right { pad } else { 0 };
        Paragraph::new(Line::from(text))
            .render(Rect { x, y: area.y, width: at.width - pad, height: 1 }, buf);
    }
    place.separators(cx, buf, area.x, area.y, Style::new());
}

/// The width a column's title takes in the header, with room for the sort marker if it can sort.
pub(crate) fn title_width(column: &Column) -> usize {
    column.title.width() + usize::from(column.sort.is_some() && !column.title.is_empty())
}
