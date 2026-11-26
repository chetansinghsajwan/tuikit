//! The grid: state, keys and drawing.

use crate::table::{self, Table};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use std::path::PathBuf;
use tui_input::Input;
use ui::app::{Navigable, Request};
use ui::footer::Footer;
use ui::prompt::Typed;
use ui::table::{Cell, Column, Row, Width};
use ui::{Component, Pane};

/// The widest a column is drawn.
const MAX_COL: u16 = 32;
/// The narrowest a column is drawn, so empty columns stay visible.
const MIN_COL: u16 = 3;
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

/// The table editor: the table, the cursor and scroll (a `ui::table::Table` in cell mode), undo history
/// and mode.
pub struct Grid {
    table: Table,
    path: Option<PathBuf>,
    header: bool,
    /// The cell cursor, the scroll both ways and the column widths.
    view: ui::table::Table,
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
        let mut grid = Grid {
            saved: table.clone(),
            table,
            path,
            header: true,
            view: ui::table::Table::new().with_cells(),
            undo: Vec::new(),
            redo: Vec::new(),
            mode: Mode::Normal,
            find: String::new(),
            copied: String::new(),
            message: None,
        };
        grid.clamp();
        grid
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
        (self.view.cursor(), self.view.col())
    }

    /// Puts the cursor on the cell (row, column), or the nearest one in the table.
    fn set_cursor(&mut self, (row, col): (usize, usize)) {
        self.clamp();
        self.view.set_cursor(row);
        self.view.set_col(col);
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

    /// The column count, at least 1 so the cursor always has a place.
    fn cols(&self) -> usize {
        self.table.col_count().max(1)
    }

    /// Remember the table before a change. The column widths start over, as the cells change.
    fn change(&mut self) {
        self.undo.push((self.table.clone(), self.cursor()));
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.view.reset_widths();
    }

    /// Tells the view the table's size, which pulls the cursor back inside after a change shrank it.
    fn clamp(&mut self) {
        self.view.set_len(self.table.row_count());
        self.view.set_cols(self.cols());
    }

    /// Handles a key press for the open prompt or question; in normal mode it does nothing.
    fn typing(&mut self, ev: &Event) -> Request {
        let Some(key) = ui::keys::pressed(ev) else { return Request::None };
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Edit(mut input) => {
                match ui::prompt::typed(&mut input, ev) {
                    Typed::Submit(text) => {
                        self.change();
                        let (r, c) = self.cursor();
                        self.table.set(r, c, &text);
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
                self.table.delete_row(self.cursor().0);
                self.clamp();
                self.show_message("deleted the row", false);
            }
            Confirm::DeleteCol => {
                self.change();
                self.table.delete_col(self.cursor().1);
                self.clamp();
                self.show_message("deleted the column", false);
            }
            Confirm::Quit => return Request::Quit,
        }
        Request::None
    }

    /// Runs one of the editor's own actions by id.
    fn run_action_id(&mut self, id: &'static str) -> Request {
        let (r, c) = self.cursor();
        match id {
            "nav.left" => self.view.move_col_by(-1),
            "nav.right" => self.view.move_col_by(1),
            "nav.first_col" => self.view.set_col(0),
            "nav.last_col" => self.view.set_col(self.cols() - 1),
            "cell.edit" => self.mode = Mode::Edit(Input::new(self.table.value(r, c))),
            "cell.clear" => {
                self.change();
                self.table.set(r, c, "");
            }
            "cell.copy" => {
                self.copied = self.table.value(r, c);
                let _ = ui::clipboard::copy(&self.copied);
                self.show_message("copied", false);
            }
            "cell.paste" => {
                self.change();
                let text = self.copied.clone();
                self.table.set(r, c, &text);
            }
            "row.add" => {
                self.change();
                self.table.insert_row(r + 1);
                self.set_cursor((r + 1, c));
            }
            "row.add_above" => {
                self.change();
                self.table.insert_row(r);
            }
            "row.delete" => self.mode = Mode::Confirm(Confirm::DeleteRow),
            "row.up" if r > 0 => {
                self.change();
                self.table.move_row(r, r - 1);
                self.set_cursor((r - 1, c));
            }
            "row.down" if r + 1 < self.table.row_count() => {
                self.change();
                self.table.move_row(r, r + 1);
                self.set_cursor((r + 1, c));
            }
            "col.add" => {
                self.change();
                self.table.insert_col(c + 1);
                self.set_cursor((r, c + 1));
            }
            "col.delete" => self.mode = Mode::Confirm(Confirm::DeleteCol),
            "col.left" if c > 0 => {
                self.change();
                self.table.move_col(c, c - 1);
                self.set_cursor((r, c - 1));
            }
            "col.right" if c + 1 < self.table.col_count() => {
                self.change();
                self.table.move_col(c, c + 1);
                self.set_cursor((r, c + 1));
            }
            "sort.asc" | "sort.desc" => {
                self.change();
                self.table.sort_by(c, id == "sort.desc", self.header);
                self.show_message(format!("sorted by column {}", c + 1), false);
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
                    self.redo.push((std::mem::replace(&mut self.table, t), (r, c)));
                    self.view.reset_widths();
                    self.set_cursor(cur);
                }
                None => self.show_message("nothing to undo", true),
            },
            "redo" => match self.redo.pop() {
                Some((t, cur)) => {
                    self.undo.push((std::mem::replace(&mut self.table, t), (r, c)));
                    self.view.reset_widths();
                    self.set_cursor(cur);
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
        let (r, c) = self.cursor();
        let at = r * cols + c;
        for step in 1..=total {
            let i = if forward { (at + step) % total } else { (at + total - step % total) % total };
            let (r, c) = (i / cols, i % cols);
            if self.table.value(r, c).to_lowercase().contains(&needle) {
                self.set_cursor((r, c));
                return;
            }
        }
        self.show_message(format!("{} not found", self.find), true);
    }

    /// Draws the table with a row-number gutter, the header and a footer, scrolling to keep the
    /// cursor in view.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, cx: &ui::Context, focused: bool) {
        let theme = cx.theme;
        let (cur_row, cur_col) = self.cursor();
        let c = &theme.colors;
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "table".into());
        let title = if self.modified() { format!("{name} [+]") } else { name.clone() };
        let pane = Pane::new().title(&title).icon(theme.icons.file(&name, false)).focused(focused);
        let mut inner = pane.draw(cx, area, buf);
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
                Confirm::DeleteRow => format!("Delete row {}? d again", cur_row + 1),
                Confirm::DeleteCol => format!("Delete column {}? d again", cur_col + 1),
                Confirm::Quit => "Unsaved changes. q again to discard".to_string(),
            }),
            Mode::Normal => match &self.message {
                Some(notice) => Footer::Notice(notice),
                None => Footer::Hint(format!(
                    "R{} C{}  {} rows × {} cols{}",
                    cur_row + 1,
                    cur_col + 1,
                    self.table.row_count(),
                    self.table.col_count(),
                    if self.modified() { "  modified" } else { "" }
                )),
            },
        };
        footer.render(cx, footer_row, buf);

        // The view draws the rows: the row numbers, the columns (as wide as their cells, within
        // bounds) with a `│` between, the cursor cell and its row, the header row pinned on top.
        let header = self.header && self.table.row_count() > 0;
        let columns: Vec<Column> = (0..self.table.col_count())
            .map(|_| Column::new("col", "", Width::Fit { max: MAX_COL }).min(MIN_COL))
            .collect();
        let table = &self.table;
        let row = |r: usize| {
            let is_head = header && r == 0;
            let style = if is_head {
                Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(c.fg.into())
            };
            let cells = (0..table.col_count())
                .map(|col| {
                    let value = table.value(r, col);
                    let numeric = !value.is_empty() && value.trim().parse::<f64>().is_ok();
                    let cell = Cell::from(Span::styled(value.replace('\n', "⏎"), style));
                    if numeric && !is_head { cell.right() } else { cell }
                })
                .collect();
            Row::new(cells)
        };
        self.view
            .view(table.row_count(), &columns, row)
            .pane(area)
            .focused(focused)
            .numbered(true)
            .separator(true)
            .pinned(|first| if header && first > 0 { vec![0] } else { Vec::new() })
            .empty("empty table (a adds a row)")
            .render(cx, inner, buf);
    }
}

/// Lets the shared app shell host the grid: it owns the keymap and the overlays, the grid draws
/// and runs its own actions.
impl ui::app::Screen for Grid {
    /// Draws the table, always focused since it is the only pane.
    fn render(&mut self, cx: &ui::Context, area: Rect, buf: &mut Buffer) {
        Grid::render(self, area, buf, cx, true);
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
        self.view.move_by(delta);
    }

    fn page_row_count(&self) -> usize {
        self.view.page_row_count()
    }

    fn move_to_top(&mut self) {
        self.view.move_to_top();
    }

    fn move_to_bottom(&mut self) {
        self.view.move_to_bottom();
    }
}

/// Tests for the grid.
#[cfg(test)]
mod tests;
