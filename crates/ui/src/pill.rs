//! Pills: a short label on a coloured background between rounded caps, for inline code and other
//! small chips. The caps are Powerline glyphs, so they need a Nerd Font like the icons do.

use crate::icons::IconProvider;
use ratatui::style::Style;
use ratatui::text::Span;

/// `text` as one word that `word_spans` draws as a pill. It stays a single unbreakable word, so it
/// can go through code that wraps and measures plain strings (the caps are one column each).
pub fn word(icons: &IconProvider, text: &str) -> String {
    format!("{}{text}{}", icons.pill_left.glyph, icons.pill_right.glyph)
}

/// The spans for a word that was made by `word`: left cap, the text on `style`, right cap, and
/// whatever glued on after it (punctuation) unstyled. A word that is not a pill, or a style with no
/// background to round, is one span in `style`.
pub fn word_spans(icons: &IconProvider, word: &str, style: Style) -> Vec<Span<'static>> {
    let (Some(bg), Some(inner)) = (style.bg, word.strip_prefix(icons.pill_left.glyph)) else {
        return vec![Span::styled(word.to_string(), style)];
    };
    let Some((text, rest)) = inner.split_once(icons.pill_right.glyph) else {
        return vec![Span::styled(word.to_string(), style)];
    };
    // The caps are the pill's colour drawn as the glyph itself, on the terminal's own background.
    let cap = Style::new().fg(bg);
    let mut spans = vec![
        Span::styled(icons.pill_left.glyph, cap),
        Span::styled(text.to_string(), style),
        Span::styled(icons.pill_right.glyph, cap),
    ];
    if !rest.is_empty() {
        spans.push(Span::raw(rest.to_string()));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn icons() -> IconProvider {
        IconProvider::default()
    }

    #[test]
    fn a_pill_is_cap_text_cap() {
        let style = Style::new().fg(Color::Red).bg(Color::Blue);
        let spans = word_spans(&icons(), &word(&icons(), "code"), style);
        let shown: Vec<&str> = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(shown, ["", "code", ""]);
        assert_eq!(spans[0].style.fg, Some(Color::Blue), "the cap is the pill's colour");
        assert_eq!(spans[0].style.bg, None, "on the terminal's own background");
        assert_eq!(spans[1].style, style);
    }

    #[test]
    fn what_glues_on_after_a_pill_is_plain() {
        let style = Style::new().bg(Color::Blue);
        let spans = word_spans(&icons(), &format!("{}.", word(&icons(), "x")), style);
        assert_eq!(spans.last().map(|s| s.content.as_ref()), Some("."));
        assert_eq!(spans.last().map(|s| s.style), Some(Style::default()));
    }

    #[test]
    fn other_words_and_unfilled_styles_stay_one_span() {
        let style = Style::new().bg(Color::Blue);
        assert_eq!(word_spans(&icons(), "plain", style).len(), 1);
        assert_eq!(word_spans(&icons(), &word(&icons(), "x"), Style::new()).len(), 1);
    }
}
