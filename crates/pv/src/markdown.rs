//! Rendered Markdown: headings, emphasis, lists, quotes, code (highlighted), tables, images,
//! alerts, footnotes, front matter, math and rules drawn as styled terminal lines (the markup
//! characters are gone), wrapped to the width.

use pulldown_cmark::{Alignment, BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::collections::HashMap;
use std::path::Path;
use ui::Theme;
use unicode_width::UnicodeWidthStr;

/// Whether `path` is a Markdown file by its extension.
pub fn is_markdown(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| matches!(e.to_str().map(str::to_ascii_lowercase).as_deref(), Some("md" | "markdown")))
}

/// The tallest an inline image gets, in rows.
const IMAGE_ROWS: u16 = 16;
/// The widest an inline image gets, in columns.
const IMAGE_COLS: u16 = 64;

/// The parser options every Markdown view uses.
pub fn options() -> Options {
    Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_GFM
        | Options::ENABLE_MATH
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
}

/// A rendered document: its lines and the rows where the headings start.
pub struct Rendered {
    pub lines: Vec<Line<'static>>,
    pub headings: Vec<usize>,
}

type Words = Vec<(String, Style)>;

/// A table being collected: the cells (as styled words) of every row, the head row apart.
struct Table {
    aligns: Vec<Alignment>,
    head: Vec<Words>,
    rows: Vec<Vec<Words>>,
    row: Vec<Words>,
    in_head: bool,
}

struct Out {
    width: usize,
    lines: Vec<Line<'static>>,
    headings: Vec<usize>,
    /// The words of the paragraph being built, with their styles.
    words: Words,
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
    table: Option<Table>,
    /// The image being read: its target and the alt text collected so far.
    image: Option<(String, String)>,
    /// Front matter being read.
    meta: Option<String>,
    /// The footnotes in the order they were first named.
    notes: HashMap<String, usize>,
    /// Where relative image paths start.
    base: Option<std::path::PathBuf>,
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

    /// A line of the current block: the prefix, then `spans`.
    fn push_line(&mut self, spans: Vec<Span<'static>>) {
        let mut full = self.indent.clone();
        full.extend(spans);
        self.lines.push(Line::from(full));
    }

    /// Wrap the collected words into lines under the current prefix.
    fn flush(&mut self) {
        if self.words.is_empty() {
            return;
        }
        let avail = self.width.saturating_sub(self.prefix_width() + 2).max(10);
        let words = std::mem::take(&mut self.words);
        let mut first = true;
        for spans in wrap(&words, avail) {
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

    /// The number of a footnote, counted from one in the order the notes are named.
    fn note(&mut self, name: &str) -> usize {
        let next = self.notes.len() + 1;
        *self.notes.entry(name.to_string()).or_insert(next)
    }
}

/// `words` wrapped to lines of at most `avail` columns (a word longer than that gets a line of its own).
fn wrap(words: &[(String, Style)], avail: usize) -> Vec<Vec<Span<'static>>> {
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
        line.push(Span::styled(word.clone(), *style));
        used += w;
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn line_width(spans: &[Span<'static>]) -> usize {
    spans.iter().map(|s| s.content.width()).sum()
}

/// The column widths of a table that has to fit in `room` columns: as wide as the content, the widest
/// columns giving way first (never under six).
fn column_widths(natural: &[usize], room: usize) -> Vec<usize> {
    let mut w: Vec<usize> = natural.iter().map(|&n| n.max(3)).collect();
    while w.iter().sum::<usize>() > room {
        let (at, widest) = w.iter().enumerate().max_by_key(|(_, v)| **v).map(|(i, v)| (i, *v)).unwrap_or((0, 0));
        if widest <= 6 {
            break;
        }
        w[at] -= 1;
    }
    w
}

/// The lines of a table: bordered, the head row bold, cells wrapped, aligned as the table says.
fn table_lines(t: &Table, theme: &Theme, room: usize) -> Vec<Vec<Span<'static>>> {
    let c = &theme.colors;
    let (muted, accent) = (Style::new().fg(c.muted.into()), Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD));
    let n = t.aligns.len().max(t.head.len()).max(t.rows.iter().map(Vec::len).max().unwrap_or(0));
    if n == 0 {
        return Vec::new();
    }
    let text_of = |cell: &Words| cell.iter().map(|(w, _)| w.as_str()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
    let mut natural = vec![0usize; n];
    for row in std::iter::once(&t.head).chain(t.rows.iter()) {
        for (i, cell) in row.iter().enumerate() {
            natural[i] = natural[i].max(text_of(cell).width());
        }
    }
    let widths = column_widths(&natural, room.saturating_sub(3 * n + 1));
    let border = |l: &str, m: &str, r: &str| -> Vec<Span<'static>> {
        let body: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
        vec![Span::styled(format!("{l}{}{r}", body.join(m)), muted)]
    };
    let mut out = vec![border("┌", "┬", "┐")];
    let emit = |row: &Vec<Words>, head: bool, out: &mut Vec<Vec<Span<'static>>>| {
        let cells: Vec<Vec<Vec<Span<'static>>>> = (0..n)
            .map(|i| {
                let words: Words = row.get(i).map(|c| c.iter().map(|(w, s)| (w.clone(), if head { accent } else { *s })).collect()).unwrap_or_default();
                let lines = wrap(&words, widths[i]);
                if lines.is_empty() { vec![Vec::new()] } else { lines }
            })
            .collect();
        for k in 0..cells.iter().map(Vec::len).max().unwrap_or(1) {
            let mut spans = vec![Span::styled("│", muted)];
            for (i, cell) in cells.iter().enumerate() {
                let content = cell.get(k).cloned().unwrap_or_default();
                let pad = widths[i].saturating_sub(line_width(&content));
                let (left, right) = match t.aligns.get(i) {
                    Some(Alignment::Right) => (pad, 0),
                    Some(Alignment::Center) => (pad / 2, pad - pad / 2),
                    _ => (0, pad),
                };
                spans.push(Span::raw(format!(" {}", " ".repeat(left))));
                spans.extend(content);
                spans.push(Span::raw(format!("{} ", " ".repeat(right))));
                spans.push(Span::styled("│", muted));
            }
            out.push(spans);
        }
    };
    if !t.head.is_empty() {
        emit(&t.head, true, &mut out);
        out.push(border("├", "┼", "┤"));
    }
    for row in &t.rows {
        emit(row, false, &mut out);
    }
    out.push(border("└", "┴", "┘"));
    out
}

/// The color and name of an alert (`> [!NOTE]`).
fn alert(kind: BlockQuoteKind, theme: &Theme) -> (Color, &'static str) {
    let c = &theme.colors;
    match kind {
        BlockQuoteKind::Note => (c.info.into(), "ⓘ Note"),
        BlockQuoteKind::Tip => (c.success.into(), "💡 Tip"),
        BlockQuoteKind::Important => (c.accent.into(), "❗ Important"),
        BlockQuoteKind::Warning => (c.warning.into(), "⚠ Warning"),
        BlockQuoteKind::Caution => (c.error.into(), "⛔ Caution"),
    }
}

/// Render `text` as Markdown for `width` columns.
pub fn render(text: &str, theme: &Theme, width: u16) -> Vec<Line<'static>> {
    render_in(text, theme, width, None).lines
}

/// `render`, with the directory relative image paths start from, and where the headings are.
pub fn render_in(text: &str, theme: &Theme, width: u16, base: Option<&Path>) -> Rendered {
    let c = &theme.colors;
    let accent = Color::from(c.accent);
    let muted = Color::from(c.muted);
    let mut out = Out {
        width: usize::from(width.max(20)),
        lines: Vec::new(),
        headings: Vec::new(),
        words: Vec::new(),
        styles: Vec::new(),
        indent: Vec::new(),
        bullet: None,
        lists: Vec::new(),
        in_code: false,
        code: String::new(),
        code_lang: String::new(),
        table: None,
        image: None,
        meta: None,
        notes: HashMap::new(),
        base: base.map(Path::to_path_buf),
    };
    let mut link_url: Vec<String> = Vec::new();
    for event in Parser::new_ext(text, options()) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    out.blank();
                    out.headings.push(out.lines.len());
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
                Tag::BlockQuote(kind) => {
                    out.flush();
                    match kind {
                        Some(k) => {
                            let (color, title) = alert(k, theme);
                            out.indent.push(Span::styled("▏ ", Style::new().fg(color)));
                            out.push_line(vec![Span::styled(title, Style::new().fg(color).add_modifier(Modifier::BOLD))]);
                        }
                        None => out.indent.push(Span::styled("▏ ", Style::new().fg(muted))),
                    }
                }
                Tag::CodeBlock(kind) => {
                    out.flush();
                    out.in_code = true;
                    out.code.clear();
                    out.code_lang = match kind {
                        CodeBlockKind::Fenced(l) => l.split([' ', ',', '{']).next().unwrap_or("").to_string(),
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
                Tag::Image { dest_url, .. } => out.image = Some((dest_url.to_string(), String::new())),
                Tag::Table(aligns) => {
                    out.flush();
                    out.table = Some(Table { aligns, head: Vec::new(), rows: Vec::new(), row: Vec::new(), in_head: false });
                }
                Tag::TableHead => {
                    if let Some(t) = out.table.as_mut() {
                        t.in_head = true;
                    }
                }
                Tag::TableRow => {}
                Tag::TableCell => out.words.clear(),
                Tag::FootnoteDefinition(name) => {
                    out.flush();
                    out.blank();
                    let n = out.note(&name);
                    let marker = format!("[{n}] ");
                    let pad = " ".repeat(marker.width());
                    out.bullet = Some(Span::styled(marker, Style::new().fg(muted)));
                    out.indent.push(Span::raw(pad));
                }
                Tag::MetadataBlock(_) => out.meta = Some(String::new()),
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
                    let code = code.trim_end_matches('\n');
                    let inner = out.width.saturating_sub(out.prefix_width() + 2).max(10);
                    let tint = ui::mix(c.fg.into(), c.bg.into(), 7);
                    if !lang.is_empty() {
                        out.push_line(vec![Span::styled(format!("  {lang}"), Style::new().fg(muted).add_modifier(Modifier::ITALIC))]);
                    }
                    let lit = Style::new().fg(Color::from(c.syntax.string));
                    for pieces in crate::text::highlight_code(code, &lang, theme) {
                        // Long lines are cut at the width, the block is tinted to it.
                        let mut spans = vec![Span::styled(" ", Style::new().bg(tint))];
                        let mut used = 1;
                        for (style, piece) in pieces {
                            if used >= inner {
                                break;
                            }
                            let piece: String = piece.chars().scan(0usize, |w, ch| { *w += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0); Some((*w, ch)) }).take_while(|(w, _)| used + w <= inner).map(|(_, ch)| ch).collect();
                            used += piece.width();
                            spans.push(Span::styled(piece, style.bg(tint)));
                        }
                        spans.push(Span::styled(" ".repeat(inner.saturating_sub(used) + 1), Style::new().bg(tint)));
                        out.push_line(spans);
                    }
                    if code.is_empty() {
                        out.push_line(vec![Span::styled("  ", lit)]);
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
                TagEnd::Image => {
                    if let Some((url, alt)) = out.image.take() {
                        let local = if url.contains("://") { None } else { out.base.as_ref().map(|b| b.join(&url)).filter(|p| p.is_file()) };
                        let picture = local.filter(|p| crate::media::kind(p) == Some(crate::media::Kind::Image)).and_then(|p| {
                            let cols = (out.width.saturating_sub(out.prefix_width() + 1) as u16).min(IMAGE_COLS);
                            crate::media::image_rows(&p, theme, cols, IMAGE_ROWS).ok()
                        });
                        match picture {
                            Some(rows) => {
                                out.flush();
                                for row in rows {
                                    out.push_line(row.spans);
                                }
                                if !alt.is_empty() {
                                    out.push_line(vec![Span::styled(alt, Style::new().fg(muted).add_modifier(Modifier::ITALIC))]);
                                }
                            }
                            None => {
                                let shown = if alt.is_empty() { url.clone() } else { format!("{alt} ({url})") };
                                out.words.push((format!("🖼 {shown}"), Style::new().fg(muted)));
                            }
                        }
                    }
                }
                TagEnd::TableCell => {
                    let cell = std::mem::take(&mut out.words);
                    if let Some(t) = out.table.as_mut() {
                        t.row.push(cell);
                    }
                }
                TagEnd::TableHead => {
                    if let Some(t) = out.table.as_mut() {
                        t.head = std::mem::take(&mut t.row);
                        t.in_head = false;
                    }
                }
                TagEnd::TableRow => {
                    if let Some(t) = out.table.as_mut() {
                        let row = std::mem::take(&mut t.row);
                        t.rows.push(row);
                    }
                }
                TagEnd::Table => {
                    if let Some(t) = out.table.take() {
                        let room = out.width.saturating_sub(out.prefix_width());
                        for spans in table_lines(&t, theme, room) {
                            out.push_line(spans);
                        }
                        out.lines.push(Line::default());
                    }
                }
                TagEnd::FootnoteDefinition => {
                    out.flush();
                    out.indent.pop();
                    out.bullet = None;
                }
                TagEnd::MetadataBlock(_) => {
                    if let Some(meta) = out.meta.take() {
                        let rule = Span::styled("┈".repeat(out.width.saturating_sub(out.prefix_width()).min(60)), Style::new().fg(muted));
                        out.push_line(vec![rule.clone()]);
                        for l in meta.trim_end().lines() {
                            out.push_line(vec![Span::styled(l.to_string(), Style::new().fg(muted))]);
                        }
                        out.push_line(vec![rule]);
                        out.lines.push(Line::default());
                    }
                }
                _ => {}
            },
            Event::Text(t) => {
                if out.in_code {
                    out.code.push_str(&t);
                } else if let Some(meta) = out.meta.as_mut() {
                    meta.push_str(&t);
                } else if let Some((_, alt)) = out.image.as_mut() {
                    alt.push_str(&t);
                } else {
                    out.push_text(&t);
                }
            }
            Event::Code(t) => {
                if let Some((_, alt)) = out.image.as_mut() {
                    alt.push_str(&t);
                } else {
                    let style = Style::new().fg(Color::from(c.syntax.string)).bg(Color::from(c.selection));
                    out.words.push((format!("{t}"), style));
                }
            }
            Event::InlineMath(t) => out.words.push((t.to_string(), Style::new().fg(Color::from(c.info)).add_modifier(Modifier::ITALIC))),
            Event::DisplayMath(t) => {
                out.flush();
                for l in t.trim().lines() {
                    out.push_line(vec![Span::styled(format!("  {l}"), Style::new().fg(Color::from(c.info)).add_modifier(Modifier::ITALIC))]);
                }
                out.lines.push(Line::default());
            }
            Event::FootnoteReference(name) => {
                let n = out.note(&name);
                match out.words.last_mut() {
                    Some(last) if !last.0.is_empty() => last.0.push_str(&format!("[{n}]")),
                    _ => out.words.push((format!("[{n}]"), Style::new().fg(accent))),
                }
            }
            Event::Html(h) | Event::InlineHtml(h) => {
                // Comments are for the author; other markup shows as it is, muted.
                if !h.trim_start().starts_with("<!--") {
                    for l in h.lines().filter(|l| !l.trim().is_empty()) {
                        out.words.push((l.trim().to_string(), Style::new().fg(muted)));
                    }
                }
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
        }
    }
    out.flush();
    while out.lines.last().is_some_and(|l| l.spans.is_empty()) {
        out.lines.pop();
    }
    Rendered { lines: out.lines, headings: out.headings }
}

/// A piece of Markdown syntax that a live view hides (`replace` None) or shows as `replace`, until
/// the cursor is in `element` (the heading, emphasis, link or item the syntax belongs to).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conceal {
    pub range: std::ops::Range<usize>,
    pub element: std::ops::Range<usize>,
    pub replace: Option<&'static str>,
}

/// The syntax to hide in `text` (byte ranges): heading marks, emphasis, strikethrough and code
/// marks, link brackets and targets, and list bullets (shown as `•`).
pub fn conceal(text: &str) -> Vec<Conceal> {
    let mut out = Vec::new();
    let mut hide = |range: std::ops::Range<usize>, element: &std::ops::Range<usize>, replace| {
        if range.start < range.end {
            out.push(Conceal { range, element: element.clone(), replace });
        }
    };
    // The extent of what is inside each open link.
    let mut links: Vec<Option<std::ops::Range<usize>>> = Vec::new();
    for (event, r) in Parser::new_ext(text, Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS).into_offset_iter() {
        let is_link_edge = matches!(event, Event::Start(Tag::Link { .. }) | Event::End(TagEnd::Link));
        if !is_link_edge {
            if let Some(slot) = links.last_mut() {
                match slot {
                    Some(inner) => {
                        inner.start = inner.start.min(r.start);
                        inner.end = inner.end.max(r.end);
                    }
                    None => *slot = Some(r.clone()),
                }
            }
        }
        let head = &text[r.start..r.end.min(text.len())];
        match event {
            Event::Start(Tag::Heading { .. }) if head.starts_with('#') => {
                let hashes = head.len() - head.trim_start_matches('#').len();
                let spaces = head[hashes..].len() - head[hashes..].trim_start_matches(' ').len();
                let line_end = head.find('\n').map_or(r.end, |n| r.start + n);
                hide(r.start..r.start + hashes + spaces, &(r.start..line_end), None);
            }
            Event::End(TagEnd::Strong) | Event::End(TagEnd::Emphasis) | Event::End(TagEnd::Strikethrough) => {
                let n = match event {
                    Event::End(TagEnd::Strong) => 2,
                    Event::End(TagEnd::Strikethrough) if head.starts_with("~~") => 2,
                    _ => 1,
                };
                if head.len() >= 2 * n {
                    hide(r.start..r.start + n, &r, None);
                    hide(r.end - n..r.end, &r, None);
                }
            }
            Event::Code(_) => {
                let n = head.len() - head.trim_start_matches('`').len();
                if n > 0 && head.len() > 2 * n {
                    hide(r.start..r.start + n, &r, None);
                    hide(r.end - n..r.end, &r, None);
                }
            }
            Event::Start(Tag::Link { .. }) => links.push(None),
            Event::End(TagEnd::Link) => {
                if let Some(Some(inner)) = links.pop() {
                    if head.starts_with('[') && inner.start > r.start && inner.end < r.end {
                        hide(r.start..inner.start, &r, None);
                        hide(inner.end..r.end, &r, None);
                    }
                }
            }
            Event::Start(Tag::Item) => {
                if head.starts_with(['-', '*', '+']) && head[1..].starts_with(' ') {
                    let line_end = head.find('\n').map_or(r.end, |n| r.start + n);
                    hide(r.start..r.start + 1, &(r.start..line_end), Some("•"));
                }
            }
            _ => {}
        }
    }
    out.sort_by_key(|c| c.range.start);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()).collect()
    }

    #[test]
    fn conceal_finds_the_syntax_to_hide() {
        let src = "# Title\n\nsome **bold** and *it* and `code` and [a link](http://x.y) end\n\n- item\n";
        let hidden = |c: &Conceal| src[c.range.clone()].to_string();
        let all = conceal(src);
        let marks: Vec<String> = all.iter().map(hidden).collect();
        assert_eq!(marks, ["# ", "**", "**", "*", "*", "`", "`", "[", "](http://x.y)", "-"]);
        // The element of a mark is what the cursor has to be in to show it.
        let bold = all.iter().find(|c| hidden(c) == "**").unwrap();
        assert_eq!(&src[bold.element.clone()], "**bold**");
        let bullet = all.last().unwrap();
        assert_eq!(bullet.replace, Some("•"));
        // A heading mark is shown while the cursor is on its line.
        assert_eq!(&src[all[0].element.clone()], "# Title");
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
    fn tables_are_bordered_aligned_and_fit_the_width() {
        let md = "| name | n |\n|:-----|--:|\n| alpha | 1 |\n| b | 22 |\n";
        let lines = text(&render(md, &Theme::default(), 40));
        assert_eq!(lines[0], "┌───────┬─────┐", "{lines:#?}");
        assert!(lines[1].contains("name") && lines[1].starts_with('│'), "{lines:#?}");
        assert!(lines.iter().any(|l| l == "│ alpha │   1 │"), "numbers align right: {lines:#?}");
        assert!(lines.last().unwrap().starts_with('└'));
        // Too wide: the columns shrink and the cells wrap, nothing sticks out.
        let wide = format!("| a | b |\n|---|---|\n| {} | y |\n", "word ".repeat(30));
        let lines = text(&render(&wide, &Theme::default(), 40));
        assert!(lines.iter().all(|l| l.width() <= 40), "{lines:#?}");
        assert!(lines.len() > 6, "the long cell wrapped: {lines:#?}");
    }

    #[test]
    fn alerts_footnotes_front_matter_math_and_headings() {
        let md = "---\ntitle: T\n---\n\n# One\n\n> [!WARNING]\n> careful\n\nA note[^a] and $x^2$.\n\n[^a]: the footnote\n\n## Two\n";
        let r = render_in(md, &Theme::default(), 60, None);
        let all = text(&r.lines).join("\n");
        assert!(all.contains("title: T") && all.contains("┈"), "{all}");
        assert!(all.contains("⚠ Warning") && all.contains("▏ careful"), "{all}");
        assert!(all.contains("A note[1] and x^2."), "{all}");
        assert!(all.contains("[1] the footnote"), "{all}");
        assert_eq!(r.headings.len(), 2);
        assert!(text(&r.lines)[r.headings[1]].contains("Two"));
    }

    #[test]
    fn code_blocks_are_highlighted_and_images_fall_back_to_their_alt_text() {
        let lines = render("```rust\nfn main() {}\n```\n\n![a cat](missing.png)\n", &Theme::default(), 50);
        let code = lines.iter().find(|l| l.spans.iter().any(|s| s.content.contains("fn"))).unwrap();
        let colors: std::collections::HashSet<_> = code.spans.iter().filter_map(|s| s.style.fg).collect();
        assert!(colors.len() > 1, "the keyword and the rest differ in color");
        assert!(text(&lines).join("\n").contains("🖼 a cat (missing.png)"));
        // A real image next to the file is drawn.
        let d = std::env::temp_dir().join(format!("pv-md-img-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 0, 0, 255])).save(d.join("p.png")).unwrap();
        let r = render_in("![red](p.png)\n", &Theme::default(), 50, Some(&d));
        assert!(r.lines.iter().any(|l| l.spans.iter().any(|s| s.content == "▀")), "{:?}", text(&r.lines));
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
