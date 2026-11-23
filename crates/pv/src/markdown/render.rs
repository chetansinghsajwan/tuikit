use super::state::Out;
use super::{Rendered, options};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::text::Line;
use std::collections::HashMap;
use std::path::Path;
use ui::Theme;

/// Render `text` as Markdown for `width` columns.
pub fn render(text: &str, theme: &Theme, width: u16) -> Vec<Line<'static>> {
    render_in(text, theme, width, None).lines
}

/// `render`, with the directory relative image paths start from, and where the headings are.
pub fn render_in(text: &str, theme: &Theme, width: u16, base: Option<&Path>) -> Rendered {
    let mut out = Out {
        width: usize::from(width.max(20)),
        icons: theme.icons,
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
        links: Vec::new(),
        base: base.map(Path::to_path_buf),
    };
    for event in Parser::new_ext(text, options()) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => out.start_heading(level, theme),
                Tag::Paragraph => {}
                Tag::BlockQuote(kind) => out.start_quote(kind, theme),
                Tag::CodeBlock(kind) => out.start_code_block(kind),
                Tag::List(start) => out.start_list(start),
                Tag::Item => out.start_item(theme),
                Tag::Emphasis => out.start_emphasis(),
                Tag::Strong => out.start_strong(),
                Tag::Strikethrough => out.start_strikethrough(),
                Tag::Link { dest_url, .. } => out.start_link(&dest_url, theme),
                Tag::Image { dest_url, .. } => out.start_image(&dest_url),
                Tag::Table(aligns) => out.start_table(aligns),
                Tag::TableHead => out.start_table_head(),
                Tag::TableRow => {}
                Tag::TableCell => out.start_table_cell(),
                Tag::FootnoteDefinition(name) => out.start_footnote(&name, theme),
                Tag::MetadataBlock(_) => out.start_front_matter(),
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(level) => out.end_heading(level),
                TagEnd::Paragraph => out.end_paragraph(),
                TagEnd::BlockQuote(_) => out.end_quote(),
                TagEnd::CodeBlock => out.end_code_block(theme),
                TagEnd::List(_) => out.end_list(),
                TagEnd::Item => out.end_item(),
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => out.end_style(),
                TagEnd::Link => out.end_link(theme),
                TagEnd::Image => out.end_image(theme),
                TagEnd::TableCell => out.end_table_cell(),
                TagEnd::TableHead => out.end_table_head(),
                TagEnd::TableRow => out.end_table_row(),
                TagEnd::Table => out.end_table(theme),
                TagEnd::FootnoteDefinition => out.end_footnote(),
                TagEnd::MetadataBlock(_) => out.end_front_matter(theme),
                _ => {}
            },
            Event::Text(t) => out.text(&t),
            Event::Code(t) => out.inline_code(&t, theme),
            Event::InlineMath(t) => out.inline_math(&t, theme),
            Event::DisplayMath(t) => out.display_math(&t, theme),
            Event::FootnoteReference(name) => out.footnote_reference(&name, theme),
            Event::Html(h) | Event::InlineHtml(h) => out.html(&h, theme),
            Event::SoftBreak => out.soft_break(),
            Event::HardBreak => out.hard_break(),
            Event::Rule => out.rule(theme),
            Event::TaskListMarker(done) => out.task_marker(done, theme),
        }
    }
    out.flush();
    while out.lines.last().is_some_and(|l| l.spans.is_empty()) {
        out.lines.pop();
    }
    Rendered { lines: out.lines, headings: out.headings }
}
