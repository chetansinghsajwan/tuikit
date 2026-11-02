//! Rendered Markdown: headings, emphasis, lists, quotes, code and rules drawn as styled
//! terminal lines (the markup characters are gone), wrapped to the width.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ui::Theme;
use unicode_width::UnicodeWidthStr;

/// Whether `path` is a Markdown file by its extension.
pub fn is_markdown(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| matches!(e.to_str().map(str::to_ascii_lowercase).as_deref(), Some("md" | "markdown")))
}

struct Out {
    width: usize,
    lines: Vec<Line<'static>>,
    /// The words of the paragraph being built, with their styles.
    words: Vec<(String, Style)>,
    /// Style stack for emphasis, strong, links and so on.
    styles: Vec<Style>,
    /// Prefix for continuation lines of the current block (list indent, quote bar).
    indent: Vec<Span<'static>>,
    /// First-line prefix override (a list bullet), used once.
    bullet: Option<Span<'static>>,
    /// Open lists: `Some(n)` numbered (next number), `None` bulleted.
    lists: Vec<Option<u64>>,
    in_code: bool,
    code: String,
    code_lang: String,
}

impl Out {
    fn style(&self) -> Style {
        self.styles.last().copied().unwrap_or_default()
    }

    fn prefix_width(&self) -> usize {
        self.indent.iter().map(|s| s.content.width()).sum()
    }

    fn blank(&mut self) {
        if self.lines.last().is_some_and(|l| !l.spans.is_empty()) {
            self.lines.push(Line::default());
        }
    }

    /// Wrap the collected words into lines under the current prefix.
    fn flush(&mut self) {
        if self.words.is_empty() {
            return;
        }
        let avail = self.width.saturating_sub(self.prefix_width() + 2).max(10);
        let mut line: Vec<Span<'static>> = Vec::new();
        let mut used = 0;
        let words = std::mem::take(&mut self.words);
        let mut first = true;
        let emit = |this: &mut Self, spans: Vec<Span<'static>>, first: &mut bool| {
            let mut full = Vec::new();
            if *first {
                full.extend(this.indent.iter().take(this.indent.len().saturating_sub(1)).cloned());
                match this.bullet.take() {
                    Some(b) => full.push(b),
                    None => full.extend(this.indent.last().cloned()),
                }
                *first = false;
            } else {
                full.extend(this.indent.iter().cloned());
            }
            full.extend(spans);
            this.lines.push(Line::from(full));
        };
        for (word, style) in words {
            if word.is_empty() {
                continue;
            }
            let w = word.width();
            if used > 0 && used + w > avail {
                let done = std::mem::take(&mut line);
                emit(self, done, &mut first);
                used = 0;
            }
            // A space belongs before every word but the first on a line.
            if used > 0 {
                line.push(Span::raw(" "));
                used += 1;
            }
            line.push(Span::styled(word, style));
            used += w;
        }
        if !line.is_empty() {
            emit(self, line, &mut first);
        }
    }

    /// Add inline text: words join the previous word when nothing separates them, and a soft break
    /// (an empty word) separates the next one.
    fn push_text(&mut self, text: &str) {
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
}

/// Render `text` as Markdown for `width` columns.
pub fn render(text: &str, theme: &Theme, width: u16) -> Vec<Line<'static>> {
    let c = &theme.colors;
    let accent = Color::from(c.accent);
    let muted = Color::from(c.muted);
    let mut out = Out {
        width: usize::from(width.max(20)),
        lines: Vec::new(),
        words: Vec::new(),
        styles: Vec::new(),
        indent: Vec::new(),
        bullet: None,
        lists: Vec::new(),
        in_code: false,
        code: String::new(),
        code_lang: String::new(),
    };
    let mut link_url: Vec<String> = Vec::new();
    for event in Parser::new_ext(text, Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    out.blank();
                    let mark = match level {
                        HeadingLevel::H1 => "█ ",
                        HeadingLevel::H2 => "▌ ",
                        _ => "▎ ",
                    };
                    out.words.push((mark.trim_end().to_string(), Style::new().fg(muted)));
                    out.words.push((String::new(), Style::new()));
                    out.styles.push(Style::new().fg(accent).add_modifier(Modifier::BOLD));
                }
                Tag::Paragraph => {}
                Tag::BlockQuote(_) => out.indent.push(Span::styled("▏ ", Style::new().fg(muted))),
                Tag::CodeBlock(kind) => {
                    out.flush();
                    out.in_code = true;
                    out.code.clear();
                    out.code_lang = match kind {
                        CodeBlockKind::Fenced(l) => l.to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                }
                Tag::List(start) => out.lists.push(start),
                Tag::Item => {
                    out.flush();
                    let depth = out.lists.len().saturating_sub(1);
                    let marker = match out.lists.last_mut() {
                        Some(Some(n)) => {
                            let m = format!("{n}. ");
                            *n += 1;
                            m
                        }
                        _ => ["• ", "◦ ", "▪ "][depth % 3].to_string(),
                    };
                    let pad = " ".repeat(marker.width());
                    out.bullet = Some(Span::styled(marker, Style::new().fg(accent)));
                    out.indent.push(Span::raw(pad));
                }
                Tag::Emphasis => out.styles.push(out.style().add_modifier(Modifier::ITALIC)),
                Tag::Strong => out.styles.push(out.style().add_modifier(Modifier::BOLD)),
                Tag::Strikethrough => out.styles.push(out.style().add_modifier(Modifier::CROSSED_OUT)),
                Tag::Link { dest_url, .. } => {
                    link_url.push(dest_url.to_string());
                    out.styles.push(out.style().fg(Color::from(c.info)).add_modifier(Modifier::UNDERLINED));
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    out.flush();
                    out.styles.pop();
                    out.lines.push(Line::default());
                }
                TagEnd::Paragraph => {
                    out.flush();
                    if out.lists.is_empty() || out.indent.is_empty() {
                        out.lines.push(Line::default());
                    }
                }
                TagEnd::BlockQuote(_) => {
                    out.flush();
                    out.indent.pop();
                }
                TagEnd::CodeBlock => {
                    out.in_code = false;
                    let code = std::mem::take(&mut out.code);
                    let lang = std::mem::take(&mut out.code_lang);
                    if !lang.is_empty() {
                        out.lines.push(Line::from(vec![Span::raw(" ".repeat(out.prefix_width())), Span::styled(format!("  {lang}"), Style::new().fg(muted).add_modifier(Modifier::ITALIC))]));
                    }
                    for l in code.trim_end_matches('\n').lines() {
                        let mut spans = out.indent.clone();
                        spans.push(Span::styled(format!("  {}", l.replace('\t', "    ")), Style::new().fg(Color::from(c.syntax.string))));
                        out.lines.push(Line::from(spans));
                    }
                    out.lines.push(Line::default());
                }
                TagEnd::List(_) => {
                    out.lists.pop();
                    if out.lists.is_empty() {
                        out.lines.push(Line::default());
                    }
                }
                TagEnd::Item => {
                    out.flush();
                    out.indent.pop();
                    out.bullet = None;
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                    out.styles.pop();
                }
                TagEnd::Link => {
                    out.styles.pop();
                    if let Some(url) = link_url.pop() {
                        out.words.push((format!("({url})"), Style::new().fg(muted)));
                    }
                }
                _ => {}
            },
            Event::Text(t) => {
                if out.in_code {
                    out.code.push_str(&t);
                } else {
                    out.push_text(&t);
                }
            }
            Event::Code(t) => {
                let style = Style::new().fg(Color::from(c.syntax.string)).bg(Color::from(c.selection));
                out.words.push((format!("{t}"), style));
            }
            Event::SoftBreak => out.words.push((String::new(), out.style())),
            Event::HardBreak => out.flush(),
            Event::Rule => {
                out.flush();
                let w = out.width.saturating_sub(out.prefix_width()).min(60);
                out.lines.push(Line::from(Span::styled("─".repeat(w), Style::new().fg(muted))));
                out.lines.push(Line::default());
            }
            Event::TaskListMarker(done) => {
                out.bullet = Some(Span::styled(if done { "☑ " } else { "☐ " }, Style::new().fg(accent)));
            }
            _ => {}
        }
    }
    out.flush();
    while out.lines.last().is_some_and(|l| l.spans.is_empty()) {
        out.lines.pop();
    }
    out.lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()).collect()
    }

    #[test]
    fn headings_lists_quotes_and_code_lose_their_markup() {
        let md = "# Title\n\nSome *emphasis* and `code` here.\n\n- one\n- two\n\n1. first\n\n> quoted\n\n```rust\nlet x = 1;\n```\n";
        let lines = text(&render(md, &Theme::default(), 60));
        let all = lines.join("\n");
        assert!(all.contains("Title") && !all.contains("# Title"), "{all}");
        assert!(all.contains("Some emphasis and code here."), "{all}");
        assert!(all.contains("• one") && all.contains("• two") && all.contains("1. first"), "{all}");
        assert!(all.contains("▏ quoted"), "{all}");
        assert!(all.contains("let x = 1;") && !all.contains("```"), "{all}");
    }

    #[test]
    fn paragraphs_wrap_to_the_width() {
        let md = "word ".repeat(40);
        let lines = text(&render(&md, &Theme::default(), 30));
        assert!(lines.len() > 3 && lines.iter().all(|l| l.width() <= 30), "{lines:?}");
    }

    #[test]
    fn links_show_their_target() {
        let all = text(&render("see [the docs](https://x.y/z) now", &Theme::default(), 80)).join("\n");
        assert!(all.contains("the docs (https://x.y/z) now"), "{all}");
    }
}
