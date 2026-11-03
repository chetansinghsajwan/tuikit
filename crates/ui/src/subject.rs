//! Commit subjects: the category before the colon (`fix:`, `feat(ui)!:`) in a color of its own.

use crate::theme::Theme;
use ratatui::style::{Color, Style};
use ratatui::text::Span;

/// The length of the category prefix (`feat(ui)!:` included) when the subject starts with one.
pub fn category_len(subject: &str) -> Option<usize> {
    let colon = subject.find(':')?;
    let head = &subject[..colon];
    let word_end = head.find(['(', '!']).unwrap_or(head.len());
    let word = &head[..word_end];
    if word.is_empty() || !word.chars().all(|c| c.is_ascii_alphabetic() || c == '-' || c == '_') {
        return None;
    }
    // After the word only an optional `(scope)` and an optional `!` may come.
    let rest = &head[word_end..];
    let rest = match rest.strip_prefix('(') {
        Some(r) => r.split_once(')').map(|(scope, r)| (!scope.contains(['(', ' ']) || !scope.is_empty()).then_some(r)).flatten()?,
        None => rest,
    };
    (rest.is_empty() || rest == "!").then_some(colon + 1)
}

/// The category word of a subject, lowercase and without scope or `!`: `feat(ui)!: x` -> `feat`.
pub fn category_word(subject: &str) -> Option<String> {
    let n = category_len(subject)?;
    subject[..n].trim_end_matches(':').split(['(', '!']).next().map(str::to_lowercase)
}

/// The color of a category: one per word, the same word always the same color (a hash picks it from the theme's colors).
pub fn category_color(theme: &Theme, subject: &str) -> Color {
    let c = &theme.colors;
    let palette: [Color; 8] = [c.accent.into(), c.success.into(), c.warning.into(), c.error.into(), c.info.into(), c.git.modified.into(), c.git.renamed.into(), c.git.untracked.into()];
    let word = category_word(subject).unwrap_or_default();
    let hash = word.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3));
    palette[(hash % palette.len() as u64) as usize]
}

/// The subject as spans: the category in its color (bold), the rest in `base`.
pub fn spans(theme: &Theme, subject: &str, base: Style) -> Vec<Span<'static>> {
    match category_len(subject) {
        Some(n) => vec![Span::styled(subject[..n].to_string(), Style::new().fg(category_color(theme, subject)).add_modifier(ratatui::style::Modifier::BOLD)), Span::styled(subject[n..].to_string(), base)],
        None => vec![Span::styled(subject.to_string(), base)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_category_prefix() {
        assert_eq!(category_len("update: some message"), Some(7));
        assert_eq!(category_len("feat(ui)!: big"), Some(10));
        assert_eq!(category_len("fix(a): b"), Some(7));
        assert_eq!(category_len("not a category: no"), None);
        assert_eq!(category_len("no colon"), None);
        assert_eq!(category_len(": empty"), None);
        assert_eq!(category_len("a/b: path"), None);
        assert_eq!(category_len("fix(: odd"), None);
    }

    #[test]
    fn colors_by_kind_and_splits_the_spans() {
        let t = Theme::default();
        assert_eq!(category_color(&t, "fix: x"), category_color(&t, "fix(ui)!: y"));
        assert_eq!(category_color(&t, "Fix: x"), category_color(&t, "fix: x"));
        assert_eq!(category_word("Feat(ui)!: a").as_deref(), Some("feat"));
        assert_eq!(category_word("plain"), None);
        let s = spans(&t, "docs: write", Style::new());
        assert_eq!((s[0].content.as_ref(), s[1].content.as_ref()), ("docs:", " write"));
        assert_eq!(spans(&t, "plain subject", Style::new()).len(), 1);
    }
}
