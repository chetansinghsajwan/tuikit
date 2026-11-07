//! List rows: the one place that decides how a row of a list is laid out and
//! highlighted, so every tool's lists (files, pick, git, diagnostics) look alike.
//!
//! A row's *bar* (its background) spans the pane's padding on both sides, so the
//! highlight touches the borders; its *content* stays inside the padding, giving
//! the text a one-cell margin within the bar.

use crate::theme::{Theme, mix};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug, Clone, Copy)]
pub struct Rows {
    inner: Rect,
    left: u16,
    right: u16,
}

impl Rows {
    /// `pane` is the whole pane, `inner` the list's area inside its border and padding.
    pub fn new(pane: Rect, inner: Rect) -> Rows {
        let left = inner.x.saturating_sub(pane.x + 1).min(1);
        let right = pane.right().saturating_sub(inner.right() + 1).min(1);
        Rows { inner, left, right }
    }

    /// Which list this is, for the motion: where its content starts.
    pub fn id(&self) -> (u16, u16) {
        (self.inner.x, self.inner.y)
    }

    /// The row's background at `y`, reaching into the padding on both sides.
    pub fn bar(&self, y: u16) -> Rect {
        Rect {
            x: self.inner.x - self.left,
            y,
            width: self.inner.width + self.left + self.right,
            height: 1,
        }
    }

    /// The row's content area at `y`, inside the padding.
    pub fn content(&self, y: u16) -> Rect {
        Rect { y, height: 1, ..self.inner }
    }
}

/// The style of a row's bar: a faded blue (the accent) for the hovered row, a faded
/// tint of `tint` (a git color, say) for a row that has one, nothing otherwise.
pub fn style(theme: &Theme, hovered: bool, tint: Option<Color>) -> Style {
    let c = &theme.colors;
    match (hovered, tint) {
        (true, _) => {
            Style::new().bg(mix(c.accent.into(), c.bg.into(), 28)).add_modifier(Modifier::BOLD)
        }
        (false, Some(t)) => Style::new().bg(mix(t, c.bg.into(), 14)),
        (false, None) => Style::new(),
    }
}

/// `text` cut to at most `width` cells, with a `…` marking the cut. The one rule for text that does not fit:
/// the name (of a file, a folder, a ref) is the part that shrinks, so what sits beside it stays where it is.
/// Counts cells, not chars, so wide characters can not push the columns out of line.
pub fn fit(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let (mut out, mut used) = (String::new(), 0);
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > width - 1 {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}

/// One row of any list: the single component every tool's list rows are drawn with (files, commits, pick
/// results, diagnostics, symbols, networks). The tool passes the content and what it wants; the row decides
/// the bar and highlight, the room (the left content shrinks with a `…`, the right column and the signs keep
/// their place, one blank cell always between them) and reports itself to `anim` so scrolling, folds and
/// changes move smoothly. Nothing here is the tool's to repeat.
#[derive(Default)]
pub struct ListRow {
    /// What identifies the row across frames (its path, hash, name) and what it shows apart from the
    /// cursor, for the motion. Leave both 0 and the text of `left` and `right` stands in.
    pub id: u64,
    pub content: u64,
    /// The content, left to right. The end of it gives way when the row is narrow.
    pub left: Vec<Span<'static>>,
    /// A column right-aligned at the end (sizes, times, counts); dropped when it would leave `min_left` cells.
    pub right: Vec<Span<'static>>,
    pub min_left: usize,
    /// Signs at the right edge, the most serious first (git letters).
    pub signs: Vec<(char, Color)>,
    /// The last cell is kept for signs even when this row has none, so right columns line up across rows.
    pub sign_cell: bool,
    pub tint: Option<Color>,
    pub selected: bool,
    /// The pane has the focus; a selected row of an unfocused pane is the faint current row.
    pub focused: bool,
}

fn text_of(spans: &[Span]) -> String {
    spans.iter().map(|s| s.content.as_ref()).collect()
}

/// `spans` cut to `limit` cells, with a `…` where the cut falls.
pub fn clip_spans(spans: Vec<Span<'static>>, limit: usize) -> Vec<Span<'static>> {
    if spans.iter().map(|s| s.content.width()).sum::<usize>() <= limit {
        return spans;
    }
    let mut out = Vec::new();
    let mut used = 0;
    for s in spans {
        let w = s.content.width();
        if used + w <= limit.saturating_sub(1) {
            used += w;
            out.push(s);
            continue;
        }
        let room = limit.saturating_sub(used);
        if room > 0 {
            out.push(Span::styled(fit(&s.content, room), s.style));
        }
        break;
    }
    out
}

impl ListRow {
    /// Draw the row at line `y` and report it to the motion.
    pub fn draw(self, theme: &Theme, buf: &mut ratatui::buffer::Buffer, rows: &Rows, y: u16) {
        let (id, content) = if self.id == 0 && self.content == 0 {
            (
                crate::anim::hash_of(&text_of(&self.left)),
                crate::anim::hash_of(&text_of(&self.right)),
            )
        } else {
            (self.id, self.content)
        };
        self.paint(theme, buf, rows, y);
        crate::anim::note_row(rows.id(), y, id, content, rows.bar(y));
    }

    /// Draw the row without the motion: for a row built off screen (a preview).
    pub fn paint(self, theme: &Theme, buf: &mut ratatui::buffer::Buffer, rows: &Rows, y: u16) {
        use ratatui::text::Line;
        use ratatui::widgets::{Paragraph, Widget};
        let base = if self.selected && !self.focused {
            active(theme)
        } else {
            style(theme, self.selected && self.focused, self.tint)
        };
        buf.set_style(rows.bar(y), base);
        let area = rows.content(y);
        let width = usize::from(area.width);
        let cell = usize::from(self.sign_cell);
        let signs = if self.signs.is_empty() { cell } else { self.signs.len() * 2 };
        let right_w: usize = self.right.iter().map(|s| s.content.width()).sum();
        // One blank cell between the left content and the right column, and the column clear of the signs.
        let with_right = signs.max(cell + right_w + 1);
        let show_right = right_w > 0 && width.saturating_sub(with_right) >= self.min_left;
        let limit = width.saturating_sub(if show_right { with_right } else { signs });
        let left = clip_spans(self.left, limit);
        Paragraph::new(Line::from(left))
            .style(base)
            .render(Rect { width: limit as u16, ..area }, buf);
        if show_right {
            let x = area.right().saturating_sub((cell + right_w) as u16);
            let right: Vec<Span> = self
                .right
                .iter()
                .map(|s| {
                    Span::styled(
                        s.content.to_string(),
                        Style::new().fg(theme.colors.muted.into()).patch(s.style),
                    )
                })
                .collect();
            Paragraph::new(Line::from(right))
                .style(base)
                .render(Rect { x, width: right_w as u16, ..area }, buf);
        }
        let mut x = area.right().saturating_sub(1);
        for (i, (ch, color)) in self.signs.iter().enumerate().rev() {
            buf.set_string(x, area.y, ch.to_string(), base.fg(*color));
            x = x.saturating_sub(if i == 0 { 0 } else { 2 });
        }
    }
}

/// The row that is current while its pane does not have the focus (the file open in the editor): a
/// fainter blue than the hovered row.
pub fn active(theme: &Theme) -> Style {
    let c = &theme.colors;
    Style::new().bg(mix(c.accent.into(), c.bg.into(), 14)).add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests;
