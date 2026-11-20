//! The grid: state, keys and drawing.

use crate::table::{self, Table};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;
use std::path::PathBuf;
use tui_input::Input;
use ui::app::{Navigable, Request};
use ui::footer::Footer;
use ui::prompt::Typed;
use ui::{Pane, Theme};
use unicode_width::UnicodeWidthStr;

/// The widest a column is drawn.
const MAX_COL: usize = 32;
/// The narrowest a column is drawn, so empty columns stay visible.
const MIN_COL: usize = 3;
/// How many rows are measured for the width of a column.
const MEASURE_ROWS: usize = 2000;
/// How many changes undo remembers, to bound memory on big tables.
const UNDO_DEPTH: usize = 100;

/// What the keys do now: move around, type into a cell or the find box, or answer a question.
enum Mode {
    Normal,
    Edit(Input),
    Find(Input),
    Confirm(Confirm),
}

/// The risky actions that ask for a second key press first.
#[derive(Clone, Copy)]
enum Confirm {
    DeleteRow,
    DeleteCol,
    Quit,
}

/// The table editor: the table, the cursor, the scroll position, undo history and mode.
pub struct Grid {
    table: Table,
    path: Option<PathBuf>,
    header: bool,
    /// The cell the cursor is in.
    cur: (usize, usize),
    top: usize,
    left: usize,
    /// How many body rows fit (set when drawn).
    page: usize,
    undo: Vec<(Table, (usize, usize))>,
    redo: Vec<(Table, (usize, usize))>,
    saved: Table,
    mode: Mode,
    find: String,
    copied: String,
    message: Option<ui::footer::Notice>,
}

impl Grid {
    /// Makes an editor for `table`, saved to `path` (if any).
    pub fn new(table: Table, path: Option<PathBuf>) -> Grid {
        Grid {
            saved: table.clone(),
            table,
            path,
            header: true,
            cur: (0, 0),
            top: 0,
            left: 0,
            page: 10,
            undo: Vec::new(),
            redo: Vec::new(),
            mode: Mode::Normal,
            find: String::new(),
            copied: String::new(),
            message: None,
        }
    }

    /// Reads a csv, tsv or psv file into an editor; a missing file starts as an empty table.
    pub fn open(path: &std::path::Path) -> Result<Grid, String> {
        let delim = table::delimiter(path)
            .ok_or_else(|| format!("{}: not a csv, tsv or psv file", path.display()))?;
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        Ok(Grid::new(Table::parse(&text, delim), Some(path.to_path_buf())))
    }

    /// The table being edited.
    pub fn table(&self) -> &Table {
        &self.table
    }

    /// The cell the cursor is in, as (row, column).
    pub fn cursor(&self) -> (usize, usize) {
        self.cur
    }

    /// Whether the table differs from what was last saved.
    pub fn modified(&self) -> bool {
        self.table != self.saved
    }

    /// Tells the host that keys go to a text prompt, so it must not treat them as its own.
    pub fn is_typing(&self) -> bool {
        matches!(self.mode, Mode::Edit(_) | Mode::Find(_))
    }

    /// Shows a message in the footer; `error` colours it as a failure.
    fn show_message(&mut self, text: impl Into<String>, error: bool) {
        self.message = Some(ui::footer::Notice::new(text, error));
    }

    /// The row count, at least 1 so the cursor always has a place.
    fn rows(&self) -> usize {
        self.table.row_count().max(1)
    }

    /// The column count, at least 1 so the cursor always has a place.
    fn cols(&self) -> usize {
        self.table.col_count().max(1)
    }

    /// Remember the table before a change.
    fn change(&mut self) {
        self.undo.push((self.table.clone(), self.cur));
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Pulls the cursor back inside the table after a change shrank it.
    fn clamp(&mut self) {
        self.cur.0 = self.cur.0.min(self.rows() - 1);
        self.cur.1 = self.cur.1.min(self.cols() - 1);
    }

    /// Moves the cursor by `delta` rows, stopping at the first and last.
    fn go_rows(&mut self, delta: isize) {
        self.cur.0 = ui::list::move_cursor(self.cur.0, delta, self.rows());
    }

    /// Moves the cursor by `delta` columns, stopping at the first and last.
    fn go_cols(&mut self, delta: isize) {
        self.cur.1 = ui::list::move_cursor(self.cur.1, delta, self.cols());
    }

    /// Handles a key press for the open prompt or question; in normal mode it does nothing.
    fn typing(&mut self, ev: &Event) -> Request {
        let Some(key) = ui::keys::pressed(ev) else { return Request::None };
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Edit(mut input) => {
                match ui::prompt::typed(&mut input, ev) {
                    Typed::Submit(text) => {
                        self.change();
                        self.table.set(self.cur.0, self.cur.1, &text);
                    }
                    Typed::Cancel => {}
                    Typed::Editing => self.mode = Mode::Edit(input),
                }
                return Request::None;
            }
            Mode::Find(mut input) => {
                match ui::prompt::typed(&mut input, ev) {
                    Typed::Submit(text) => {
                        self.find = text;
                        self.find_next(true);
                    }
                    Typed::Cancel => {}
                    Typed::Editing => self.mode = Mode::Find(input),
                }
                return Request::None;
            }
            Mode::Confirm(what) => {
                let yes = ui::prompt::confirms(key.code)
                    && !matches!((what, key.code), (Confirm::Quit, KeyCode::Char('d')));
                let quit_again = matches!((what, key.code), (Confirm::Quit, KeyCode::Char('q')));
                if yes || quit_again {
                    return self.confirmed(what);
                }
                return Request::None;
            }
            Mode::Normal => {}
        }
        Request::None
    }

    /// Carries out a risky action after the user pressed the key a second time.
    fn confirmed(&mut self, what: Confirm) -> Request {
        match what {
            Confirm::DeleteRow => {
                self.change();
                self.table.delete_row(self.cur.0);
                self.clamp();
                self.show_message("deleted the row", false);
            }
            Confirm::DeleteCol => {
                self.change();
                self.table.delete_col(self.cur.1);
                self.clamp();
                self.show_message("deleted the column", false);
            }
            Confirm::Quit => return Request::Quit,
        }
        Request::None
    }

    /// Runs one of the editor's own actions by id.
    fn run_action_id(&mut self, id: &'static str) -> Request {
        match id {
            "nav.left" => self.go_cols(-1),
            "nav.right" => self.go_cols(1),
            "nav.first_col" => self.cur.1 = 0,
            "nav.last_col" => self.cur.1 = self.cols() - 1,
            "cell.edit" => {
                self.mode = Mode::Edit(Input::new(self.table.value(self.cur.0, self.cur.1)))
            }
            "cell.clear" => {
                self.change();
                self.table.set(self.cur.0, self.cur.1, "");
            }
            "cell.copy" => {
                self.copied = self.table.value(self.cur.0, self.cur.1);
                let _ = ui::clipboard::copy(&self.copied);
                self.show_message("copied", false);
            }
            "cell.paste" => {
                self.change();
                let text = self.copied.clone();
                self.table.set(self.cur.0, self.cur.1, &text);
            }
            "row.add" => {
                self.change();
                self.table.insert_row(self.cur.0 + 1);
                self.cur.0 += 1;
            }
            "row.add_above" => {
                self.change();
                self.table.insert_row(self.cur.0);
            }
            "row.delete" => self.mode = Mode::Confirm(Confirm::DeleteRow),
            "row.up" if self.cur.0 > 0 => {
                self.change();
                self.table.move_row(self.cur.0, self.cur.0 - 1);
                self.cur.0 -= 1;
            }
            "row.down" if self.cur.0 + 1 < self.table.row_count() => {
                self.change();
                self.table.move_row(self.cur.0, self.cur.0 + 1);
                self.cur.0 += 1;
            }
            "col.add" => {
                self.change();
                self.table.insert_col(self.cur.1 + 1);
                self.cur.1 += 1;
            }
            "col.delete" => self.mode = Mode::Confirm(Confirm::DeleteCol),
            "col.left" if self.cur.1 > 0 => {
                self.change();
                self.table.move_col(self.cur.1, self.cur.1 - 1);
                self.cur.1 -= 1;
            }
            "col.right" if self.cur.1 + 1 < self.table.col_count() => {
                self.change();
                self.table.move_col(self.cur.1, self.cur.1 + 1);
                self.cur.1 += 1;
            }
            "sort.asc" | "sort.desc" => {
                self.change();
                self.table.sort_by(self.cur.1, id == "sort.desc", self.header);
                self.show_message(format!("sorted by column {}", self.cur.1 + 1), false);
            }
            "view.header" => {
                self.header = !self.header;
                self.show_message(
                    if self.header { "first row is a header" } else { "no header" },
                    false,
                );
            }
            "find.start" => self.mode = Mode::Find(Input::new(self.find.clone())),
            "find.next" => self.find_next(true),
            "find.prev" => self.find_next(false),
            "undo" => match self.undo.pop() {
                Some((t, cur)) => {
                    self.redo.push((std::mem::replace(&mut self.table, t), self.cur));
                    self.cur = cur;
                    self.clamp();
                }
                None => self.show_message("nothing to undo", true),
            },
            "redo" => match self.redo.pop() {
                Some((t, cur)) => {
                    self.undo.push((std::mem::replace(&mut self.table, t), self.cur));
                    self.cur = cur;
                    self.clamp();
                }
                None => self.show_message("nothing to redo", true),
            },
            "file.save" => self.save(),
            "quit" => {
                if self.modified() {
                    self.mode = Mode::Confirm(Confirm::Quit);
                } else {
                    return Request::Quit;
                }
            }
            _ => {}
        }
        self.clamp();
        Request::None
    }

    /// Writes the table to its file and marks it as saved.
    pub fn save(&mut self) {
        let Some(path) = &self.path else {
            self.show_message("no file to save to", true);
            return;
        };
        match std::fs::write(path, self.table.to_text()) {
            Ok(()) => {
                self.saved = self.table.clone();
                self.show_message("saved", false);
            }
            Err(e) => self.show_message(format!("{}: {e}", path.display()), true),
        }
    }

    /// Jump to the next (or previous) cell, in reading order, whose text contains the find text.
    fn find_next(&mut self, forward: bool) {
        if self.find.is_empty() {
            self.show_message("nothing to find (/ to search)", true);
            return;
        }
        let needle = self.find.to_lowercase();
        let (rows, cols) = (self.table.row_count(), self.table.col_count());
        let total = rows * cols;
        let at = self.cur.0 * cols + self.cur.1;
        for step in 1..=total {
            let i = if forward { (at + step) % total } else { (at + total - step % total) % total };
            let (r, c) = (i / cols, i % cols);
            if self.table.value(r, c).to_lowercase().contains(&needle) {
                self.cur = (r, c);
                return;
            }
        }
        self.show_message(format!("{} not found", self.find), true);
    }

    /// The drawn width of each column, from its longest cell within sane bounds.
    fn widths(&self) -> Vec<usize> {
        (0..self.cols())
            .map(|c| {
                (0..self.table.row_count().min(MEASURE_ROWS))
                    .map(|r| self.table.value(r, c).replace('\n', " ").width())
                    .max()
                    .unwrap_or(0)
                    .clamp(MIN_COL, MAX_COL)
            })
            .collect()
    }

    /// Draws the table with a row-number gutter, the header and a footer, scrolling to keep the
    /// cursor in view.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme, focused: bool) {
        let c = &theme.colors;
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "table".into());
        let title = if self.modified() { format!("{name} [+]") } else { name.clone() };
        let pane =
            Pane::new(theme).title(&title).icon(theme.icons.file(&name, false)).focused(focused);
        let mut inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height < 3 || inner.width < 8 {
            return;
        }
        // The last row: a prompt, a question, a message, or where the cursor is.
        let footer_row = Rect { y: inner.bottom() - 1, height: 1, ..inner };
        inner.height -= 1;
        let footer = match &self.mode {
            Mode::Edit(i) => Footer::Prompt {
                label: "edit: ",
                text: i.value().into(),
                cursor: i.visual_cursor(),
            },
            Mode::Find(i) => Footer::Prompt {
                label: "find: ",
                text: i.value().into(),
                cursor: i.visual_cursor(),
            },
            Mode::Confirm(w) => Footer::Question(match w {
                Confirm::DeleteRow => format!("Delete row {}? d again", self.cur.0 + 1),
                Confirm::DeleteCol => format!("Delete column {}? d again", self.cur.1 + 1),
                Confirm::Quit => "Unsaved changes. q again to discard".to_string(),
            }),
            Mode::Normal => match &self.message {
                Some(notice) => Footer::Notice(notice),
                None => Footer::Hint(format!(
                    "R{} C{}  {} rows × {} cols{}",
                    self.cur.0 + 1,
                    self.cur.1 + 1,
                    self.table.row_count(),
                    self.table.col_count(),
                    if self.modified() { "  modified" } else { "" }
                )),
            },
        };
        footer.render(footer_row, buf, theme);

        let widths = self.widths();
        let digits = self.table.row_count().to_string().len().max(2);
        let gutter = digits + 1;
        let header = self.header && self.table.row_count() > 0;
        let head_rows = usize::from(header);
        self.page = (usize::from(inner.height)).saturating_sub(head_rows).max(1);

        // Scroll to keep the cursor cell on screen.
        let body_row = self.cur.0.saturating_sub(head_rows);
        if self.cur.0 >= head_rows {
            if body_row < self.top {
                self.top = body_row;
            } else if body_row >= self.top + self.page {
                self.top = body_row + 1 - self.page;
            }
        }
        let avail = usize::from(inner.width).saturating_sub(gutter);
        if self.cur.1 < self.left {
            self.left = self.cur.1;
        }
        // Move `left` right until the cursor column fits.
        while self.left < self.cur.1
            && widths[self.left..=self.cur.1].iter().map(|w| w + 3).sum::<usize>() > avail
        {
            self.left += 1;
        }

        let muted = Style::new().fg(c.muted.into());
        let numeric = |r: usize, col: usize| {
            let v = self.table.value(r, col);
            !v.is_empty() && v.trim().parse::<f64>().is_ok()
        };
        let draw_row = |r: usize, y: u16, buf: &mut Buffer| {
            let is_head = header && r == 0;
            let bar_style = if r == self.cur.0 { ui::list::active(theme) } else { Style::new() };
            let bar = Rect {
                y,
                height: 1,
                ..Rect {
                    x: inner.x.saturating_sub(1).max(area.x + 1),
                    width: inner.width + 1,
                    ..inner
                }
            };
            buf.set_style(bar, bar_style);
            buf.set_string(inner.x, y, format!("{:>w$} ", r + 1, w = digits), muted);
            let mut x = inner.x + gutter as u16;
            #[allow(clippy::needless_range_loop)] // `col` also indexes the table
            for col in self.left..self.cols() {
                if x >= inner.right() {
                    break;
                }
                let w = widths[col];
                let text = self.table.value(r, col).replace('\n', "⏎");
                let room = (inner.right() - x) as usize;
                let shown = fit(&text, w.min(room), numeric(r, col) && !is_head);
                let mut style = if is_head {
                    Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD)
                } else {
                    Style::new().fg(c.fg.into())
                };
                if (r, col) == self.cur {
                    style = ui::list::style(theme, true, None).fg(c.fg.into());
                } else if r == self.cur.0 {
                    style = style.patch(bar_style);
                }
                buf.set_string(x, y, &shown, style);
                x += w as u16;
                if x < inner.right() {
                    buf.set_string(x + 1, y, "│", muted);
                }
                x += 3;
            }
        };
        let mut y = inner.y;
        if header {
            draw_row(0, y, buf);
            y += 1;
        }
        for r in (self.top + head_rows)
            ..self.table.row_count().max(head_rows).min(self.top + head_rows + self.page)
        {
            if y >= inner.bottom() {
                break;
            }
            draw_row(r, y, buf);
            y += 1;
        }
        if self.table.row_count() == 0 {
            buf.set_string(inner.x + 1, inner.y, "empty table (a adds a row)", muted);
        }
    }
}

/// `text` cut with `…` to `width` columns, padded to it (numbers at the right).
fn fit(text: &str, width: usize, right: bool) -> String {
    let mut out = String::new();
    let mut w = 0;
    let total = text.width();
    for ch in text.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        let limit = if total > width { width.saturating_sub(1) } else { width };
        if w + cw > limit {
            break;
        }
        out.push(ch);
        w += cw;
    }
    if total > width {
        out.push('…');
        w += 1;
    }
    let pad = " ".repeat(width.saturating_sub(w));
    if right { format!("{pad}{out}") } else { format!("{out}{pad}") }
}

/// Lets the shared app shell host the grid: it owns the keymap and the overlays, the grid draws
/// and runs its own actions.
impl ui::app::Screen for Grid {
    /// Draws the table, always focused since it is the only pane.
    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        Grid::render(self, area, buf, theme, true);
    }

    /// A cell, the find box or a question is open: the keys are theirs.
    fn captures_input(&self, _ev: &Event) -> bool {
        !matches!(self.mode, Mode::Normal)
    }

    fn on_event(&mut self, ev: &Event, _host: &mut dyn ui::app::Host) -> Request {
        self.typing(ev)
    }

    fn run_action(&mut self, id: &'static str, _host: &mut dyn ui::app::Host) -> Request {
        self.run_action_id(id)
    }

    fn on_keypress(&mut self) {
        self.message = None;
    }

    /// Tells the user the key sequence they typed does nothing.
    fn on_unbound_keys(&mut self, keys: &str) {
        self.show_message(format!("{keys} is not bound"), true);
    }

    fn navigable(&mut self) -> Option<&mut dyn Navigable> {
        Some(self)
    }
}

/// The rows are what the shared movement walks.
impl Navigable for Grid {
    fn move_by(&mut self, delta: isize) {
        self.go_rows(delta);
    }

    fn page_row_count(&self) -> usize {
        self.page
    }

    fn move_to_top(&mut self) {
        self.cur.0 = 0;
    }

    fn move_to_bottom(&mut self) {
        self.cur.0 = self.rows() - 1;
    }
}

/// Tests for the grid.
#[cfg(test)]
mod tests;
