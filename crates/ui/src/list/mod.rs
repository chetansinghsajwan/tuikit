//! The rules every list and table shares: how a row's bar is colored, how text that does not fit is
//! cut, and how a cursor moves. The rows themselves are drawn by `ui::table::Table`.

use crate::theme::{Theme, mix};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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

/// The cursor `delta` rows from `cursor` in a list of `len` rows, stopping at the first and the last
/// row (0 for an empty list). The one rule every list moves by.
pub fn move_cursor(cursor: usize, delta: isize, len: usize) -> usize {
    let last = len.saturating_sub(1) as isize;
    (cursor as isize + delta).clamp(0, last) as usize
}

/// The row that is current while its pane does not have the focus (the file open in the editor): a
/// fainter blue than the hovered row.
pub fn active(theme: &Theme) -> Style {
    let c = &theme.colors;
    Style::new().bg(mix(c.accent.into(), c.bg.into(), 14)).add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests;
