//! Markdown to PDF: the Markdown is turned into Typst markup and Typst lays it out (pure Rust, no
//! other program is run). The fonts are the ones `typst-assets` embeds, so the result is the same on
//! every machine.

mod world;

use pulldown_cmark::{Alignment, BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct Export {
    /// The folder relative image paths start from (the Markdown file's folder).
    pub base: Option<PathBuf>,
}

/// The PDF of `markdown`.
pub fn to_pdf(markdown: &str, export: &Export) -> Result<Vec<u8>, String> {
    let source = typst_source(markdown, export.base.as_deref());
    world::compile(source, export.base.clone())
}

/// What the PDF is made from: the Typst markup of `markdown`.
pub fn typst_source(markdown: &str, base: Option<&Path>) -> String {
    let mut t = Typst { out: String::new(), base, lists: Vec::new(), notes: HashMap::new(), stack: Vec::new(), image_alt: None, image_src: String::new(), skip: 0 };
    t.run(markdown);
    format!("{PREAMBLE}{}", t.out)
}

const PREAMBLE: &str = r##"#set page(paper: "a4", margin: (x: 2.2cm, y: 2.4cm), numbering: "1")
#set text(font: "Libertinus Serif", size: 11pt)
#set par(justify: true, leading: 0.65em)
#set list(indent: 0.6em)
#set enum(indent: 0.6em)
#show raw: set text(font: "DejaVu Sans Mono", size: 9pt)
#show raw.where(block: true): block.with(fill: luma(244), inset: 8pt, radius: 3pt, width: 100%)
#show raw.where(block: false): box.with(fill: luma(240), inset: (x: 3pt), outset: (y: 3pt), radius: 2pt)
#show link: set text(fill: rgb("#1a5fb4"))
#show heading: set block(above: 1.4em, below: 0.8em)
#show heading.where(level: 1): set text(size: 20pt)
#show heading.where(level: 2): set text(size: 15pt)
#show heading.where(level: 3): set text(size: 12.5pt)

"##;

struct Typst<'a> {
    out: String,
    base: Option<&'a Path>,
    /// Open lists: `Some(n)` numbered (next number), `None` bulleted.
    lists: Vec<Option<u64>>,
    /// Footnote definitions by name, for the references to pick up at the end.
    notes: HashMap<String, String>,
    /// Outputs set aside while a footnote definition is read: (its name, what was written before it).
    stack: Vec<(String, String)>,
    /// The alt text of the image being read.
    image_alt: Option<String>,
    image_src: String,
    /// Depth of a block whose content is dropped (front matter).
    skip: usize,
}

/// `s` as Typst markup text: the characters that mean something are escaped.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '#' | '$' | '*' | '_' | '`' | '<' | '>' | '@' | '[' | ']' | '~' | '/') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// `s` as a Typst string literal.
fn string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn alert(kind: BlockQuoteKind) -> (&'static str, &'static str) {
    match kind {
        BlockQuoteKind::Note => ("Note", "#1a5fb4"),
        BlockQuoteKind::Tip => ("Tip", "#26a269"),
        BlockQuoteKind::Important => ("Important", "#813d9c"),
        BlockQuoteKind::Warning => ("Warning", "#c64600"),
        BlockQuoteKind::Caution => ("Caution", "#c01c28"),
    }
}

fn align(a: &Alignment) -> &'static str {
    match a {
        Alignment::Right => "right",
        Alignment::Center => "center",
        _ => "left",
    }
}

impl Typst<'_> {
    fn run(&mut self, markdown: &str) {
        let opts = Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TABLES
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_GFM
            | Options::ENABLE_MATH
            | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
        for event in Parser::new_ext(markdown, opts) {
            self.event(event);
        }
        // The footnotes go where they are named.
        let notes = std::mem::take(&mut self.notes);
        for (name, body) in notes {
            self.out = self.out.replace(&format!("\u{1}{name}\u{1}"), &format!("#footnote[{}]", body.trim()));
        }
        self.out = self.out.replace('\u{1}', "");
    }

    /// Whether this local image can be shown: it is a file inside the base folder.
    fn image_path(&self, src: &str) -> Option<String> {
        if src.contains("://") {
            return None;
        }
        let base = self.base?;
        let file = base.join(src).canonicalize().ok()?;
        let root = base.canonicalize().ok()?;
        file.strip_prefix(&root).ok()?;
        let rel = file.strip_prefix(&root).ok()?;
        matches!(rel.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref(), Some("png" | "jpg" | "jpeg" | "gif" | "svg" | "webp")).then(|| format!("/{}", rel.to_string_lossy()))
    }

    fn event(&mut self, event: Event) {
        if self.skip > 0 {
            match event {
                Event::End(TagEnd::MetadataBlock(_)) => self.skip -= 1,
                _ => {}
            }
            return;
        }
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    let n = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                    self.out.push_str(&format!("\n{} ", "=".repeat(n)));
                }
                Tag::Paragraph => {}
                Tag::BlockQuote(None) => self.out.push_str("#quote(block: true)["),
                Tag::BlockQuote(Some(kind)) => {
                    let (title, color) = alert(kind);
                    self.out.push_str(&format!("#block(width: 100%, inset: 8pt, fill: luma(246), stroke: (left: 2.5pt + rgb(\"{color}\")))[#text(fill: rgb(\"{color}\"), weight: \"bold\")[{title}] \\ "));
                }
                Tag::CodeBlock(kind) => {
                    let lang = match kind {
                        CodeBlockKind::Fenced(l) => l.split([' ', ',', '{']).next().unwrap_or("").to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    self.out.push_str(&format!("#raw(block: true, lang: {}, ", if lang.is_empty() { "none".to_string() } else { string(&lang) }));
                    self.out.push_str("\u{2}");
                }
                Tag::List(start) => self.lists.push(start),
                Tag::Item => {
                    let depth = self.lists.len().saturating_sub(1);
                    let marker = match self.lists.last_mut() {
                        Some(Some(n)) => {
                            let m = format!("{n}. ");
                            *n += 1;
                            m
                        }
                        _ => "- ".to_string(),
                    };
                    if !self.out.ends_with('\n') {
                        self.out.push('\n');
                    }
                    self.out.push_str(&format!("{}{marker}", "  ".repeat(depth)));
                }
                Tag::Emphasis => self.out.push_str("#emph["),
                Tag::Strong => self.out.push_str("#strong["),
                Tag::Strikethrough => self.out.push_str("#strike["),
                Tag::Link { dest_url, .. } => self.out.push_str(&format!("#link({})[", string(&dest_url))),
                Tag::Image { dest_url, .. } => {
                    self.image_alt = Some(String::new());
                    self.image_src = dest_url.to_string();
                }
                Tag::Table(aligns) => {
                    let list: Vec<&str> = aligns.iter().map(align).collect();
                    self.out.push_str(&format!("#table(columns: {}, align: ({},), stroke: 0.5pt + luma(180), inset: 6pt,\n", aligns.len().max(1), list.join(", ")));
                }
                Tag::TableHead => self.out.push_str("table.header("),
                Tag::TableRow => {}
                Tag::TableCell => self.out.push('['),
                Tag::FootnoteDefinition(name) => {
                    let before = std::mem::take(&mut self.out);
                    self.stack.push((name.to_string(), before));
                }
                Tag::MetadataBlock(_) => self.skip += 1,
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => self.out.push_str("\n\n"),
                TagEnd::Paragraph => {
                    self.out.push_str(if self.lists.is_empty() { "\n\n" } else { "\n" });
                }
                TagEnd::BlockQuote(_) => self.out.push_str("]\n\n"),
                TagEnd::CodeBlock => {
                    // The code was collected between the \u{2} marker and here.
                    if let Some(at) = self.out.rfind('\u{2}') {
                        let code: String = self.out[at + 1..].to_string();
                        self.out.truncate(at);
                        self.out.push_str(&string(code.trim_end_matches('\n')));
                    }
                    self.out.push_str(")\n\n");
                }
                TagEnd::List(_) => {
                    self.lists.pop();
                    if self.lists.is_empty() {
                        self.out.push('\n');
                    }
                }
                TagEnd::Item => {
                    if !self.out.ends_with('\n') {
                        self.out.push('\n');
                    }
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => self.out.push(']'),
                TagEnd::Image => {
                    let alt = self.image_alt.take().unwrap_or_default();
                    let src = std::mem::take(&mut self.image_src);
                    match self.image_path(&src) {
                        Some(path) => self.out.push_str(&format!("#box(image({}, width: 80%))", string(&path))),
                        None => self.out.push_str(&escape(&format!("[image: {}]", if alt.is_empty() { src } else { alt }))),
                    }
                }
                TagEnd::Table => self.out.push_str(")\n\n"),
                TagEnd::TableHead => self.out.push_str("),\n"),
                TagEnd::TableRow => self.out.push('\n'),
                TagEnd::TableCell => self.out.push_str("],"),
                TagEnd::FootnoteDefinition => {
                    if let Some((name, before)) = self.stack.pop() {
                        let body = std::mem::replace(&mut self.out, before);
                        self.notes.insert(name, body);
                    }
                }
                _ => {}
            },
            Event::Text(t) => {
                if let Some(alt) = self.image_alt.as_mut() {
                    alt.push_str(&t);
                } else if self.out.rfind('\u{2}').is_some() && self.in_code() {
                    self.out.push_str(&t);
                } else {
                    let at_line_start = self.out.is_empty() || self.out.ends_with('\n') || self.out.ends_with("- ");
                    let mut text = escape(&t);
                    if at_line_start && text.starts_with(['=', '-', '+']) {
                        text.insert(0, '\\');
                    }
                    self.out.push_str(&text);
                }
            }
            Event::Code(t) => {
                if let Some(alt) = self.image_alt.as_mut() {
                    alt.push_str(&t);
                } else {
                    self.out.push_str(&format!("#raw({})", string(&t)));
                }
            }
            Event::InlineMath(t) => self.out.push_str(&format!("#emph(raw({}))", string(&t))),
            Event::DisplayMath(t) => self.out.push_str(&format!("\n#align(center, emph(raw({})))\n\n", string(t.trim()))),
            Event::FootnoteReference(name) => self.out.push_str(&format!("\u{1}{name}\u{1}")),
            Event::Html(_) | Event::InlineHtml(_) => {}
            Event::SoftBreak => self.out.push(' '),
            Event::HardBreak => self.out.push_str("\\\n"),
            Event::Rule => self.out.push_str("\n#line(length: 100%, stroke: 0.5pt + luma(180))\n\n"),
            Event::TaskListMarker(done) => self.out.push_str(if done { "☑ " } else { "☐ " }),
        }
    }

    /// Whether a code block is open (its marker is the last one and nothing closed it).
    fn in_code(&self) -> bool {
        self.out.rfind('\u{2}').is_some_and(|at| !self.out[at..].contains(")\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_for_headings_emphasis_lists_code_links_and_escapes() {
        let src = typst_source("# Title\n\nSome *em* **strong** `a#b` [x](http://y.z) and 1 < 2 *.\n\n- one\n- two\n\n1. a\n2. b\n\n```rust\nlet x = \"q\";\n```\n", None);
        let body = src.split_once("\n\n").map(|_| &src[PREAMBLE.len()..]).unwrap();
        assert!(body.contains("= Title"), "{body}");
        assert!(body.contains("#emph[em] #strong[strong] #raw(\"a#b\") #link(\"http://y.z\")[x]"), "{body}");
        assert!(body.contains("1 \\< 2 \\*."), "{body}");
        assert!(body.contains("- one\n- two") && body.contains("1. a\n2. b"), "{body}");
        assert!(body.contains("#raw(block: true, lang: \"rust\", \"let x = \\\"q\\\";\")"), "{body}");
    }

    #[test]
    fn tables_quotes_alerts_and_footnotes() {
        let md = "| a | b |\n|:--|--:|\n| 1 | 2 |\n\n> quoted\n\n> [!NOTE]\n> heads up\n\nText[^n].\n\n[^n]: the note\n";
        let src = typst_source(md, None);
        assert!(src.contains("#table(columns: 2, align: (left, right,)"), "{src}");
        assert!(src.contains("table.header([a],[b],),") && src.contains("[1],[2],"), "{src}");
        assert!(src.contains("#quote(block: true)[quoted"), "{src}");
        assert!(src.contains("[Note]") && src.contains("heads up"), "{src}");
        assert!(src.contains("Text#footnote[the note]."), "{src}");
    }

    #[test]
    fn a_pdf_comes_out() {
        let md = "# Hello\n\nA paragraph with *emphasis*, `code`, a [link](https://example.org) and a list:\n\n- one\n- two\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```rust\nfn main() {}\n```\n\n> [!TIP]\n> done[^1]\n\n[^1]: note\n";
        let pdf = to_pdf(md, &Export::default()).unwrap();
        assert!(pdf.starts_with(b"%PDF-"), "not a PDF");
        assert!(pdf.len() > 2000);
    }
}
