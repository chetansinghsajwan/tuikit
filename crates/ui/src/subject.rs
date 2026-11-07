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
    // One word or two (`git panel: one tree`).
    let words: Vec<&str> = word.split(' ').collect();
    if words.len() > 2 || words.iter().any(|w| w.is_empty() || !w.chars().all(|c| c.is_ascii_alphabetic() || c == '-' || c == '_')) {
        return None;
    }
    // After the word only an optional `(scope)` and an optional `!` may come.
    let rest = &head[word_end..];
    let rest = match rest.strip_prefix('(') {
        Some(r) => r.split_once(')').and_then(|(scope, r)| (!scope.contains(['(', ' ']) || !scope.is_empty()).then_some(r))?,
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
mod tests;
