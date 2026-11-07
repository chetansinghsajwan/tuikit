//! PDF files: the title, the author and the number of pages, then the text of the first pages.

use crate::{Error, Options};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::path::Path;
use ui::Theme;

/// How many pages are read for the preview at most.
const PAGES: u32 = 6;

pub fn is_pdf(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// A text string of the document's info dictionary (`Title`, `Author`).
fn info(doc: &lopdf::Document, key: &[u8]) -> Option<String> {
    let info = doc.trailer.get(b"Info").ok()?;
    let dict = match info {
        lopdf::Object::Reference(id) => doc.get_object(*id).ok()?.as_dict().ok()?,
        other => other.as_dict().ok()?,
    };
    let value = dict.get(key).ok()?;
    let value = match value {
        lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };
    let bytes = value.as_str().ok()?;
    // UTF-16 with a byte order mark, else Latin-1 (close enough for a title).
    let text = if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        bytes.iter().map(|&b| b as char).collect()
    };
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

pub fn render(path: &Path, theme: &Theme, opts: &Options) -> Result<Vec<Line<'static>>, Error> {
    let size = std::fs::metadata(path)?.len();
    let doc = lopdf::Document::load(path).map_err(|e| Error::Unsupported(format!("pdf: {e}")))?;
    let pages = doc.get_pages();
    let c = &theme.colors;
    let (muted, fg) = (Style::new().fg(c.muted.into()), Style::new().fg(c.fg.into()));
    let mut out = Vec::new();
    if let Some(t) = info(&doc, b"Title") {
        out.push(Line::from(Span::styled(
            t,
            Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD),
        )));
    }
    let author = info(&doc, b"Author").map(|a| format!("{a} · ")).unwrap_or_default();
    out.push(Line::from(Span::styled(
        format!(
            "{author}{} page{} · {:.1} KiB",
            pages.len(),
            if pages.len() == 1 { "" } else { "s" },
            size as f64 / 1024.0
        ),
        muted,
    )));
    let width = usize::from(opts.width.unwrap_or(80)).saturating_sub(2).max(20);
    let limit = opts.height.map(usize::from).unwrap_or(usize::MAX);
    let mut text_pages = 0;
    for (&n, _) in pages.iter().take(PAGES as usize) {
        if out.len() >= limit {
            break;
        }
        let Ok(text) = doc.extract_text(&[n]) else { continue };
        let words: Vec<(String, Style)> =
            text.split_whitespace().map(|w| (w.to_string(), fg)).collect();
        if words.is_empty() {
            continue;
        }
        text_pages += 1;
        out.push(Line::default());
        out.push(Line::from(Span::styled(
            format!("── page {n} {}", "─".repeat(width.saturating_sub(10).min(50))),
            muted,
        )));
        out.extend(crate::markdown::wrap_words(&words, width).into_iter().map(Line::from));
    }
    if text_pages == 0 {
        out.push(Line::default());
        out.push(Line::from(Span::styled(
            "no text in the first pages (a scan, or text drawn as shapes)",
            muted.add_modifier(Modifier::ITALIC),
        )));
    } else if pages.len() > PAGES as usize {
        out.push(Line::default());
        out.push(Line::from(Span::styled(
            format!("… {} more pages", pages.len() - PAGES as usize),
            muted.add_modifier(Modifier::ITALIC),
        )));
    }
    out.truncate(limit.max(1));
    Ok(out)
}

#[cfg(test)]
mod tests;
