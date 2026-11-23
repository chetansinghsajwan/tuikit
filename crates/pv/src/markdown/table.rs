use super::state::Words;
use super::wrap::{clip, line_width, wrap};
use pulldown_cmark::Alignment;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ui::Theme;
use unicode_width::UnicodeWidthStr;

/// A table being collected: the cells (as styled words) of every row, the head row apart.
pub(super) struct Table {
    pub(super) aligns: Vec<Alignment>,
    pub(super) head: Vec<Words>,
    pub(super) rows: Vec<Vec<Words>>,
    pub(super) row: Vec<Words>,
    pub(super) in_head: bool,
}

/// The column widths of a table that has to fit in `room` columns: as wide as the content, the widest
/// columns giving way first (never under six).
fn column_widths(natural: &[usize], room: usize) -> Vec<usize> {
    let mut w: Vec<usize> = natural.iter().map(|&n| n.max(3)).collect();
    while w.iter().sum::<usize>() > room {
        let (at, widest) =
            w.iter().enumerate().max_by_key(|(_, v)| **v).map(|(i, v)| (i, *v)).unwrap_or((0, 0));
        if widest <= 6 {
            break;
        }
        w[at] -= 1;
    }
    w
}

/// A grid of plain strings (CSV rows, a sheet) as the lines of a bordered table; at most as many columns as fit.
/// Returns the lines and how many columns were left out. Numbers align to the right.
pub(crate) fn grid_lines(
    head: &[String],
    rows: &[Vec<String>],
    theme: &Theme,
    room: usize,
) -> (Vec<Vec<Span<'static>>>, usize) {
    let fg = Style::new().fg(theme.colors.fg.into());
    let n = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    let shown = n.min((room.saturating_sub(1) / 9).max(1));
    let words = |cell: Option<&String>| -> Words {
        cell.map(|c| c.split_whitespace().map(|w| (w.to_string(), fg)).collect())
            .unwrap_or_default()
    };
    let numeric = |i: usize| {
        let cells: Vec<&str> = rows
            .iter()
            .filter_map(|r| r.get(i))
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
            .collect();
        !cells.is_empty() && cells.iter().all(|c| c.replace(',', "").parse::<f64>().is_ok())
    };
    let table = Table {
        aligns: (0..shown)
            .map(|i| if numeric(i) { Alignment::Right } else { Alignment::Left })
            .collect(),
        head: (0..shown).map(|i| words(head.get(i))).collect(),
        rows: rows.iter().map(|r| (0..shown).map(|i| words(r.get(i))).collect()).collect(),
        row: Vec::new(),
        in_head: false,
    };
    (table_lines(&table, theme, room), n - shown)
}

/// The lines of a table: bordered, the head row bold, cells wrapped, aligned as the table says.
pub(super) fn table_lines(t: &Table, theme: &Theme, room: usize) -> Vec<Vec<Span<'static>>> {
    let c = &theme.colors;
    let (muted, accent) = (
        Style::new().fg(c.muted.into()),
        Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD),
    );
    let n = t.aligns.len().max(t.head.len()).max(t.rows.iter().map(Vec::len).max().unwrap_or(0));
    if n == 0 {
        return Vec::new();
    }
    let text_of = |cell: &Words| {
        cell.iter().map(|(w, _)| w.as_str()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
    };
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
    let mut out = vec![border("╭", "┬", "╮")];
    let emit = |row: &Vec<Words>, head: bool, out: &mut Vec<Vec<Span<'static>>>| {
        let cells: Vec<Vec<Vec<Span<'static>>>> = (0..n)
            .map(|i| {
                let words: Words = row
                    .get(i)
                    .map(|c| {
                        c.iter().map(|(w, s)| (w.clone(), if head { accent } else { *s })).collect()
                    })
                    .unwrap_or_default();
                // A word wider than its column is cut with an ellipsis.
                let words: Words = words
                    .into_iter()
                    .map(
                        |(w, s)| {
                            if w.width() > widths[i] { (clip(&w, widths[i]), s) } else { (w, s) }
                        },
                    )
                    .collect();
                let lines = wrap(&theme.icons, &words, widths[i]);
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
    out.push(border("╰", "┴", "╯"));
    out
}

impl super::state::Out {
    /// Open a table.
    pub(super) fn start_table(&mut self, aligns: Vec<Alignment>) {
        self.flush();
        self.table = Some(Table {
            aligns,
            head: Vec::new(),
            rows: Vec::new(),
            row: Vec::new(),
            in_head: false,
        });
    }

    /// Open the head row.
    pub(super) fn start_table_head(&mut self) {
        if let Some(t) = self.table.as_mut() {
            t.in_head = true;
        }
    }

    /// Open a cell.
    pub(super) fn start_table_cell(&mut self) {
        self.words.clear();
    }

    /// Close a cell.
    pub(super) fn end_table_cell(&mut self) {
        let cell = std::mem::take(&mut self.words);
        if let Some(t) = self.table.as_mut() {
            t.row.push(cell);
        }
    }

    /// Close the head row.
    pub(super) fn end_table_head(&mut self) {
        if let Some(t) = self.table.as_mut() {
            t.head = std::mem::take(&mut t.row);
            t.in_head = false;
        }
    }

    /// Close a row.
    pub(super) fn end_table_row(&mut self) {
        if let Some(t) = self.table.as_mut() {
            let row = std::mem::take(&mut t.row);
            t.rows.push(row);
        }
    }

    /// Close a table: draw it.
    pub(super) fn end_table(&mut self, theme: &Theme) {
        if let Some(t) = self.table.take() {
            let room = self.width.saturating_sub(self.prefix_width());
            for spans in table_lines(&t, theme, room) {
                self.push_line(spans);
            }
            self.lines.push(Line::default());
        }
    }
}
