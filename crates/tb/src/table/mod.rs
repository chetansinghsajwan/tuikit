//! A delimited text table (csv, tsv, psv) held as the text of its cells, so a file that was not
//! changed is written back byte for byte. No UI here: the grid editor and the editor commands of helix
//! both work on this.

use std::cmp::Ordering;
use std::path::Path;

/// A csv, tsv or psv file as rows of raw cell text, with the line ending kept so it can be written
/// back unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub delim: char,
    /// Cells as written in the file (quotes included).
    rows: Vec<Vec<String>>,
    eol: &'static str,
    final_eol: bool,
}

/// The delimiter of a file by its extension.
pub fn delimiter(path: &Path) -> Option<char> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "csv" => Some(','),
        "tsv" => Some('\t'),
        "psv" => Some('|'),
        _ => None,
    }
}

/// The text a cell shows: quotes taken off, `""` turned into `"`.
pub fn unquote(raw: &str) -> String {
    match raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        Some(inner) if raw.len() >= 2 => inner.replace("\"\"", "\""),
        _ => raw.to_string(),
    }
}

/// Cell text as it is written: quoted when it has the delimiter, a quote or a line break.
pub fn quote(value: &str, delim: char) -> String {
    if value.contains([delim, '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

impl Table {
    /// Reads `text` into rows and cells; quoted cells may hold the delimiter and line breaks.
    pub fn parse(text: &str, delim: char) -> Table {
        let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let final_eol = text.ends_with('\n');
        let mut rows: Vec<Vec<String>> = Vec::new();
        let (mut row, mut cell, mut quoted) = (Vec::new(), String::new(), false);
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '"' => {
                    cell.push(ch);
                    if quoted && chars.peek() == Some(&'"') {
                        cell.push(chars.next().unwrap_or('"'));
                    } else {
                        quoted = !quoted;
                    }
                }
                c if c == delim && !quoted => row.push(std::mem::take(&mut cell)),
                '\r' if !quoted && chars.peek() == Some(&'\n') => {}
                '\n' if !quoted => {
                    row.push(std::mem::take(&mut cell));
                    rows.push(std::mem::take(&mut row));
                }
                c => cell.push(c),
            }
        }
        if !cell.is_empty() || !row.is_empty() {
            row.push(cell);
            rows.push(row);
        }
        Table { delim, rows, eol, final_eol }
    }

    /// Writes the table back as text, using the file's own line ending and final newline.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let d = self.delim.to_string();
        for (i, row) in self.rows.iter().enumerate() {
            out.push_str(&row.join(&d));
            if i + 1 < self.rows.len() || self.final_eol {
                out.push_str(self.eol);
            }
        }
        out
    }

    /// The number of rows.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// The number of columns: the widest row, since rows may be ragged.
    pub fn col_count(&self) -> usize {
        self.rows.iter().map(Vec::len).max().unwrap_or(0)
    }

    /// The cell as written (empty outside the table).
    pub fn raw(&self, r: usize, c: usize) -> &str {
        self.rows.get(r).and_then(|row| row.get(c)).map(String::as_str).unwrap_or("")
    }

    /// The text a cell shows (quotes taken off).
    pub fn value(&self, r: usize, c: usize) -> String {
        unquote(self.raw(r, c))
    }

    /// Sets a cell to `value`, adding rows and columns when it is outside the table, and quotes it
    /// if needed.
    pub fn set(&mut self, r: usize, c: usize, value: &str) {
        while self.rows.len() <= r {
            self.rows.push(Vec::new());
        }
        let row = &mut self.rows[r];
        if row.len() <= c {
            row.resize(c + 1, String::new());
        }
        row[c] = quote(value, self.delim);
    }

    /// Inserts an empty row at `at` (or at the end when `at` is past it).
    pub fn insert_row(&mut self, at: usize) {
        let at = at.min(self.rows.len());
        let cols = self.col_count().max(1);
        self.rows.insert(at, vec![String::new(); cols]);
    }

    /// Removes row `r`; does nothing outside the table.
    pub fn delete_row(&mut self, r: usize) {
        if r < self.rows.len() {
            self.rows.remove(r);
        }
    }

    /// Moves row `from` to position `to`; does nothing outside the table.
    pub fn move_row(&mut self, from: usize, to: usize) {
        if from < self.rows.len() && to < self.rows.len() {
            let row = self.rows.remove(from);
            self.rows.insert(to, row);
        }
    }

    /// Inserts an empty column at `at` in every row, first padding short rows.
    pub fn insert_col(&mut self, at: usize) {
        let cols = self.col_count();
        for row in &mut self.rows {
            row.resize(cols, String::new());
            row.insert(at.min(cols), String::new());
        }
    }

    /// Removes column `c` from every row that has it.
    pub fn delete_col(&mut self, c: usize) {
        for row in &mut self.rows {
            if c < row.len() {
                row.remove(c);
            }
        }
    }

    /// Moves column `from` to position `to` in every row; does nothing outside the table.
    pub fn move_col(&mut self, from: usize, to: usize) {
        let cols = self.col_count();
        if from >= cols || to >= cols {
            return;
        }
        for row in &mut self.rows {
            row.resize(cols, String::new());
            let cell = row.remove(from);
            row.insert(to, cell);
        }
    }

    /// Sort the rows by a column, keeping the first row (the header) in place when `header` is set.
    /// Numbers sort as numbers and before text; empty cells go last; equal rows keep their order.
    pub fn sort_by(&mut self, col: usize, descending: bool, header: bool) {
        let skip = usize::from(header).min(self.rows.len());
        let body = &mut self.rows[skip..];
        body.sort_by(|a, b| {
            let (x, y) = (
                unquote(a.get(col).map(String::as_str).unwrap_or("")),
                unquote(b.get(col).map(String::as_str).unwrap_or("")),
            );
            match (x.is_empty(), y.is_empty()) {
                (true, true) => return Ordering::Equal,
                (true, false) => return Ordering::Greater,
                (false, true) => return Ordering::Less,
                _ => {}
            }
            let ord = match (x.trim().parse::<f64>(), y.trim().parse::<f64>()) {
                (Ok(p), Ok(q)) => p.partial_cmp(&q).unwrap_or(Ordering::Equal),
                (Ok(_), Err(_)) => Ordering::Less,
                (Err(_), Ok(_)) => Ordering::Greater,
                _ => x.to_lowercase().cmp(&y.to_lowercase()),
            };
            if descending { ord.reverse() } else { ord }
        });
    }

    /// Where the cell `(r, c)` starts in the text of the table, as a char offset.
    pub fn char_offset(&self, r: usize, c: usize) -> usize {
        let mut n = 0;
        for (i, row) in self.rows.iter().enumerate() {
            if i == r {
                return n + row.iter().take(c).map(|x| x.chars().count() + 1).sum::<usize>();
            }
            n += row.iter().map(|x| x.chars().count() + 1).sum::<usize>().max(1) - 1
                + self.eol.chars().count();
        }
        n
    }

    /// Which cell a char offset in the text falls in.
    pub fn cell_at(&self, offset: usize) -> (usize, usize) {
        let mut n = 0;
        for (i, row) in self.rows.iter().enumerate() {
            let len = row.iter().map(|x| x.chars().count() + 1).sum::<usize>().max(1) - 1;
            if offset <= n + len || i + 1 == self.rows.len() {
                let mut at = n;
                for (c, cell) in row.iter().enumerate() {
                    let w = cell.chars().count();
                    if offset <= at + w || c + 1 == row.len() {
                        return (i, c);
                    }
                    at += w + 1;
                }
                return (i, 0);
            }
            n += len + self.eol.chars().count();
        }
        (0, 0)
    }
}

/// Tests for the table.
#[cfg(test)]
mod tests;
