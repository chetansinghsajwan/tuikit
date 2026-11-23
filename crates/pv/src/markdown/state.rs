use super::table::Table;
use super::wrap::wrap;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

/// Words with the style each is drawn in; the unit that text is wrapped by.
pub(super) type Words = Vec<(String, Style)>;

/// The state of a Markdown rendering: the lines made so far and the block being read.
pub(super) struct Out {
    pub(super) width: usize,
    /// The glyphs drawn into the text (pill caps).
    pub(super) icons: ui::icons::IconProvider,
    pub(super) lines: Vec<Line<'static>>,
    pub(super) headings: Vec<usize>,
    /// The words of the paragraph being built, with their styles.
    pub(super) words: Words,
    /// Style stack for emphasis, strong, links and so on.
    pub(super) styles: Vec<Style>,
    /// Prefix for continuation lines of the current block (list indent, quote bar).
    pub(super) indent: Vec<Span<'static>>,
    /// First-line prefix override (a list bullet), used once.
    pub(super) bullet: Option<Span<'static>>,
    /// Open lists: `Some(n)` numbered (next number), `None` bulleted.
    pub(super) lists: Vec<Option<u64>>,
    pub(super) in_code: bool,
    pub(super) code: String,
    pub(super) code_lang: String,
    pub(super) table: Option<Table>,
    /// The image being read: its target and the alt text collected so far.
    pub(super) image: Option<(String, String)>,
    /// Front matter being read.
    pub(super) meta: Option<String>,
    /// The footnotes in the order they were first named.
    pub(super) notes: HashMap<String, usize>,
    /// The targets of the links being read, innermost last.
    pub(super) links: Vec<String>,
    /// Where relative image paths start.
    pub(super) base: Option<std::path::PathBuf>,
}

impl Out {
    /// The style in force now: the innermost of the emphasis, strong and link styles.
    pub(super) fn style(&self) -> Style {
        self.styles.last().copied().unwrap_or_default()
    }

    /// The width in cells of the prefix every line of the block starts with.
    pub(super) fn prefix_width(&self) -> usize {
        self.indent.iter().map(|s| s.content.width()).sum()
    }

    /// End the block with an empty line, unless the output already ends in one.
    pub(super) fn blank(&mut self) {
        if self.lines.last().is_some_and(|l| !l.spans.is_empty()) {
            self.lines.push(Line::default());
        }
    }

    /// A line of the current block: the prefix, then `spans`.
    pub(super) fn push_line(&mut self, spans: Vec<Span<'static>>) {
        let mut full = self.indent.clone();
        full.extend(spans);
        self.lines.push(Line::from(full));
    }

    /// Wrap the collected words into lines under the current prefix.
    pub(super) fn flush(&mut self) {
        if self.words.is_empty() {
            return;
        }
        let avail = self.width.saturating_sub(self.prefix_width() + 2).max(10);
        let words = std::mem::take(&mut self.words);
        let mut first = true;
        for spans in wrap(&self.icons, &words, avail) {
            let mut full = Vec::new();
            if first {
                full.extend(self.indent.iter().take(self.indent.len().saturating_sub(1)).cloned());
                match self.bullet.take() {
                    Some(b) => full.push(b),
                    None => full.extend(self.indent.last().cloned()),
                }
                first = false;
            } else {
                full.extend(self.indent.iter().cloned());
            }
            full.extend(spans);
            self.lines.push(Line::from(full));
        }
    }

    /// Add inline text: words join the previous word when nothing separates them, and a soft break
    /// (an empty word) separates the next one.
    pub(super) fn push_text(&mut self, text: &str) {
        let style = self.style();
        let mut glue = !text.starts_with(char::is_whitespace);
        for word in text.split_whitespace() {
            match self.words.last_mut() {
                Some(last) if glue && !last.0.is_empty() => last.0.push_str(word),
                _ => self.words.push((word.to_string(), style)),
            }
            glue = false;
        }
        if text.ends_with(char::is_whitespace) && !text.trim().is_empty() {
            self.words.push((String::new(), style));
        }
    }

    /// The number of a footnote, counted from one in the order the notes are named.
    pub(super) fn note(&mut self, name: &str) -> usize {
        let next = self.notes.len() + 1;
        *self.notes.entry(name.to_string()).or_insert(next)
    }
}
