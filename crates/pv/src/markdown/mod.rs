//! Rendered Markdown: headings, emphasis, lists, quotes, code (highlighted), tables, images,
//! alerts, footnotes, front matter, math and rules drawn as styled terminal lines (the markup
//! characters are gone), wrapped to the width.

use pulldown_cmark::Options;
use ratatui::text::Line;

mod block;
mod code;
mod conceal;
mod footnote;
mod frontmatter;
mod heading;
mod inline;
mod list;
mod picture;
mod quote;
mod render;
mod state;
mod table;
mod wrap;

pub use conceal::{Conceal, conceal};
pub use render::{render, render_in};
pub(crate) use table::grid_lines;
pub(crate) use wrap::wrap_words;

/// Whether `path` is a Markdown file by its extension.
pub fn is_markdown(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| {
        matches!(e.to_str().map(str::to_ascii_lowercase).as_deref(), Some("md" | "markdown"))
    })
}

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

#[cfg(test)]
mod tests;
