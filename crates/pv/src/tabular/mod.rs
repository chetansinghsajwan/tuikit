//! Tables: CSV and TSV files and spreadsheets (xlsx, xls, ods) drawn as a bordered grid, the first row
//! as the head. Only the first rows are read and shown, so a big file stays quick; `set_raw` turns it off
//! (the file then shows as text).

use crate::{Error, Options};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// How many rows are drawn.
pub const ROW_CAP: usize = 200;
/// How much of a text file is read.
const MAX_BYTES: u64 = 1 << 20;

/// The "show the raw text" switch; when on, table files are previewed as plain text.
static RAW: AtomicBool = AtomicBool::new(false);

/// Show tabular files as text instead of tables (a host's toggle).
pub fn set_raw(raw: bool) {
    RAW.store(raw, Ordering::Relaxed);
}

/// Whether tables are shown as raw text.
pub fn raw() -> bool {
    RAW.load(Ordering::Relaxed)
}

/// The kind of table file: delimited text (with its separator, or sniffed) or a spreadsheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Delimited text; the delimiter is sniffed unless the extension says it.
    Delimited(Option<u8>),
    Sheet,
}

/// What kind of table file `path` is by its extension, if any.
pub fn kind(path: &Path) -> Option<Kind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "csv" => Some(Kind::Delimited(None)),
        "tsv" | "tab" => Some(Kind::Delimited(Some(b'\t'))),
        "psv" => Some(Kind::Delimited(Some(b'|'))),
        "xlsx" | "xlsm" | "xlsb" | "xls" | "ods" => Some(Kind::Sheet),
        _ => None,
    }
}

/// The delimiter that splits the first lines into the same number of fields, the most fields winning.
pub(crate) fn sniff(text: &str) -> u8 {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(10).collect();
    let mut best = (0usize, b',');
    for d in *b",;\t|" {
        let counts: Vec<usize> =
            lines.iter().map(|l| l.bytes().filter(|&b| b == d).count()).collect();
        let first = counts.first().copied().unwrap_or(0);
        if first > 0 && counts.iter().all(|&c| c == first) && first > best.0 {
            best = (first, d);
        }
    }
    best.1
}

/// A number with commas between the thousands (`12,345`).
fn commas(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Preview of a table file as a grid, with a line of facts above it and a note for what is cut.
pub fn render(
    path: &Path,
    kind: Kind,
    cx: &ui::Context,
    opts: &Options,
) -> Result<Vec<Line<'static>>, Error> {
    let theme = cx.theme;
    let muted = Style::new().fg(theme.colors.muted.into());
    let (name, head, rows, total, more_than, extra) = match kind {
        Kind::Delimited(delim) => delimited(path, delim)?,
        Kind::Sheet => sheet(path)?,
    };
    let room = usize::from(opts.width.unwrap_or(100));
    let cap = opts
        .height
        .map(|h| usize::from(h).saturating_sub(4).max(3))
        .unwrap_or(ROW_CAP)
        .min(ROW_CAP);
    let shown_rows: Vec<Vec<String>> = rows.iter().take(cap).cloned().collect();
    let (table, hidden_cols) = crate::markdown::grid_lines(&head, &shown_rows, cx, room);
    let mut facts = format!(
        "{name} · {} column{}",
        head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0)),
        if head.len() == 1 { "" } else { "s" }
    );
    facts.push_str(&format!(
        " · {} of {}{} rows",
        commas(shown_rows.len()),
        commas(total),
        if more_than { "+" } else { "" }
    ));
    if hidden_cols > 0 {
        facts.push_str(&format!(
            " · {hidden_cols} column{} not shown",
            if hidden_cols == 1 { "" } else { "s" }
        ));
    }
    let mut out = vec![Line::from(Span::styled(facts, muted)), Line::default()];
    out.extend(table.into_iter().map(Line::from));
    if total > shown_rows.len() || more_than {
        let left = total.saturating_sub(shown_rows.len());
        out.push(Line::from(Span::styled(
            format!("… {}{} more rows", commas(left), if more_than { "+" } else { "" }),
            muted.add_modifier(Modifier::ITALIC),
        )));
    }
    if let Some(e) = extra {
        out.push(Line::default());
        out.push(Line::from(Span::styled(e, muted)));
    }
    Ok(out)
}

/// A parsed table: its label, header row, rows, total row count, whether the count is a lower
/// bound, and an extra note.
type Parsed = (String, Vec<String>, Vec<Vec<String>>, usize, bool, Option<String>);

/// Read a delimited text file (the first `MAX_BYTES` only), sniffing the separator if none
/// is given.
fn delimited(path: &Path, delim: Option<u8>) -> Result<Parsed, Error> {
    use std::io::Read;
    let mut buf = Vec::new();
    std::fs::File::open(path)?.take(MAX_BYTES).read_to_end(&mut buf)?;
    let truncated = buf.len() as u64 >= MAX_BYTES;
    if buf.iter().take(8000).any(|&b| b == 0) {
        return Err(Error::Unsupported("binary".into()));
    }
    let text = String::from_utf8_lossy(&buf);
    let d = delim.unwrap_or_else(|| sniff(&text));
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(d)
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut head: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut total = 0usize;
    for (i, rec) in reader.records().enumerate() {
        let Ok(rec) = rec else { continue };
        if i == 0 {
            head = rec.iter().map(str::to_string).collect();
            continue;
        }
        total += 1;
        if rows.len() < ROW_CAP {
            rows.push(rec.iter().map(str::to_string).collect());
        }
    }
    let name = match d {
        b'\t' => "tsv",
        b';' => "csv (;)",
        b'|' => "psv",
        _ => "csv",
    };
    // The last line of a file cut at the size limit is probably half a row.
    if truncated {
        total = total.saturating_sub(1);
    }
    Ok((name.to_string(), head, rows, total, truncated, None))
}

/// Read the first sheet of a spreadsheet, and note the other sheets.
fn sheet(path: &Path) -> Result<Parsed, Error> {
    use calamine::{Reader, open_workbook_auto};
    let mut book =
        open_workbook_auto(path).map_err(|e| Error::Unsupported(format!("spreadsheet: {e}")))?;
    let names = book.sheet_names();
    let first = names
        .first()
        .cloned()
        .ok_or_else(|| Error::Unsupported("spreadsheet without sheets".into()))?;
    let range = book
        .worksheet_range(&first)
        .map_err(|e| Error::Unsupported(format!("spreadsheet: {e}")))?;
    let cell = |c: &calamine::Data| match c {
        calamine::Data::Empty => String::new(),
        other => other.to_string(),
    };
    let mut iter = range.rows();
    let head: Vec<String> = iter.next().map(|r| r.iter().map(cell).collect()).unwrap_or_default();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut total = 0usize;
    for r in iter {
        total += 1;
        if rows.len() < ROW_CAP {
            rows.push(r.iter().map(cell).collect());
        }
    }
    let extra = (names.len() > 1).then(|| format!("other sheets: {}", names[1..].join(", ")));
    Ok((format!("sheet {first}"), head, rows, total, false, extra))
}

#[cfg(test)]
mod tests;
