use ratatui::style::Style;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

/// `words` wrapped to lines of at most `avail` columns (a word longer than that gets a line of its own).
pub(crate) fn wrap_words(
    icons: &ui::icons::IconProvider,
    words: &[(String, Style)],
    avail: usize,
) -> Vec<Vec<Span<'static>>> {
    wrap(icons, words, avail)
}

/// The wrapping behind `wrap_words`: fill each line with words, a space between them.
pub(super) fn wrap(
    icons: &ui::icons::IconProvider,
    words: &[(String, Style)],
    avail: usize,
) -> Vec<Vec<Span<'static>>> {
    let mut lines = Vec::new();
    let mut line: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    for (word, style) in words {
        if word.is_empty() {
            continue;
        }
        let w = word.width();
        if used > 0 && used + w > avail {
            lines.push(std::mem::take(&mut line));
            used = 0;
        }
        // A space belongs before every word but the first on a line.
        if used > 0 {
            line.push(Span::raw(" "));
            used += 1;
        }
        line.extend(ui::pill::word_spans(icons, word, *style));
        used += w;
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// `s` cut to `width` columns, ending in `…`.
pub(super) fn clip(s: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w + 1 > width {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}

/// The width in cells of a line of spans.
pub(super) fn line_width(spans: &[Span<'static>]) -> usize {
    spans.iter().map(|s| s.content.width()).sum()
}
