//! `pv`: preview renderer. Library returns ratatui lines so other tools can
//! embed previews in-process; the CLI prints the same lines as ANSI.

mod ansi;
mod commit;
mod config;
mod diff;
mod dir;
mod media;
mod pdf;
mod tabular;
mod git;
pub use git::kind_color;
pub mod markdown;
mod text;

use ratatui::text::Line;
use std::io;
use std::path::Path;
use ui::Theme;

pub use ansi::to_ansi;

/// One line of `code` highlighted as the language `lang` (a name or an extension), as styled pieces.
pub fn highlight_line(code: &str, lang: &str, theme: &Theme) -> Vec<(ratatui::style::Style, String)> {
    text::highlight_code(code, lang, theme).into_iter().next().unwrap_or_default()
}
pub use commit::render as preview_commit;
pub use commit::utc as format_utc;
pub use commit::render_marked as preview_commit_marked;
pub use commit::render_range_marked as preview_range_marked;
pub use commit::render_file_diff_marked as preview_diff_marked;
pub use commit::render_files_marked as preview_commit_files_marked;
pub use diff::{Anchors, ignoring_whitespace as diff_ignoring_whitespace, reset_folds as diff_reset_folds, set_ignore_whitespace as diff_ignore_whitespace, toggle_fold as diff_toggle_fold};
pub use commit::render_file_diff as preview_diff;
pub use config::Config;
pub use tabular::{raw as raw_tables, set_raw as set_raw_tables};

#[derive(Debug, Clone)]
pub struct Options {
    /// Preview width in cells (right-aligns directory metadata, pads row tints). `None` = unknown.
    pub width: Option<u16>,
    /// Visible rows. Rendering stops after this many lines. `None` = no limit.
    pub height: Option<u16>,
    /// Git status colors and signs.
    pub git: bool,
    /// 1-based line to bring into view and highlight (text files). The window
    /// is centered on it when it does not fit in `height`.
    pub focus: Option<u32>,
}

impl Default for Options {
    fn default() -> Self {
        Options { width: None, height: None, git: true, focus: None }
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    /// Not a regular file or directory.
    NotFound,
    /// Binary or otherwise unrenderable content.
    Unsupported(String),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::NotFound => write!(f, "not a file or directory"),
            Error::Unsupported(w) => write!(f, "unsupported file type ({w})"),
        }
    }
}

/// Markdown files show rendered (no markup) unless a line is to be focused, which needs the source.
fn rendered_markdown(name: &Path, text: &[u8], theme: &Theme, opts: &Options) -> Option<Vec<Line<'static>>> {
    if !markdown::is_markdown(name) || opts.focus.is_some() {
        return None;
    }
    let text = std::str::from_utf8(text).ok()?;
    let mut lines = markdown::render_in(text, theme, opts.width.unwrap_or(80), name.parent()).lines;
    if let Some(h) = opts.height {
        lines.truncate(usize::from(h));
    }
    Some(lines)
}

/// Preview text that is not (as shown) on disk, e.g. an editor buffer with
/// unsaved changes. `name` only picks the syntax; there are no git marks.
pub fn preview_text(name: &Path, text: &[u8], theme: &Theme, opts: &Options) -> Result<Vec<Line<'static>>, Error> {
    if let Some(lines) = rendered_markdown(name, text, theme, opts) {
        return Ok(lines);
    }
    text::render_bytes(name, text, theme, opts, false)
}

/// Render a preview of `path`.
pub fn preview(path: &Path, theme: &Theme, opts: &Options) -> Result<Vec<Line<'static>>, Error> {
    let meta = std::fs::metadata(path)?;
    if meta.is_dir() {
        dir::render(path, theme, opts)
    } else if meta.is_file() {
        if let Some(kind) = media::kind(path) {
            return media::render(path, kind, theme, opts);
        }
        if pdf::is_pdf(path) {
            return pdf::render(path, theme, opts);
        }
        if let Some(kind) = tabular::kind(path).filter(|_| !tabular::raw()) {
            return tabular::render(path, kind, theme, opts);
        }
        if markdown::is_markdown(path) && opts.focus.is_none() {
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut std::io::Read::take(std::fs::File::open(path)?, 1 << 20), &mut buf)?;
            if let Some(lines) = rendered_markdown(path, &buf, theme, opts) {
                return Ok(lines);
            }
        }
        text::render(path, theme, opts)
    } else {
        Err(Error::NotFound)
    }
}

#[cfg(test)]
mod tests;
