//! `diagnostics-panel`: the problems list (errors, warnings, ...) grouped by
//! file, with a severity filter. It knows nothing about any editor: the host
//! hands it a list of `Item`s (it can do so on every frame; an unchanged list
//! costs a comparison) and reacts to the `Action`s.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use std::path::{Path, PathBuf};
use ui::{Component, Pane, Theme};

/// The actions and their default keys.
mod keys;

/// Most severe first (`Error < Warning < ...` in sort order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One problem to show: where it is, how bad it is and what it says.
pub struct Item {
    /// Absolute path of the file.
    pub path: PathBuf,
    /// 1-based line and column.
    pub line: u32,
    pub col: u32,
    pub severity: Severity,
    pub message: String,
    /// Who reported it (`rust-analyzer`, `clippy`, ...).
    pub source: Option<String>,
}

/// What the host should do after an event.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    /// Go to this place.
    Open {
        path: PathBuf,
        line: u32,
        col: u32,
    },
    /// Give the keyboard back (Esc, `q`).
    Close,
    /// The palette or the help were asked for (hosts that draw them handle these).
    Palette,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Which severities the list shows. `view.severity` cycles through them.
enum Filter {
    All,
    /// Errors and warnings.
    Problems,
    Errors,
}

impl Filter {
    /// The filter that follows this one in the `f` cycle.
    fn next(self) -> Filter {
        match self {
            Filter::All => Filter::Problems,
            Filter::Problems => Filter::Errors,
            Filter::Errors => Filter::All,
        }
    }

    /// Whether a problem of severity `s` stays in the list at this filter.
    fn keeps(self, s: Severity) -> bool {
        match self {
            Filter::All => true,
            Filter::Problems => s <= Severity::Warning,
            Filter::Errors => s == Severity::Error,
        }
    }

    /// Text added to the title when the filter hides something.
    fn label(self) -> &'static str {
        match self {
            Filter::All => "",
            Filter::Problems => " [errors + warnings]",
            Filter::Errors => " [errors]",
        }
    }
}

/// One line of the list: a file heading or a problem under it.
enum Row {
    /// A file: the number of problems shown under it.
    File(PathBuf, usize),
    /// Index into `items`.
    Item(usize),
}

/// The problems list: holds the items, the rows built from them, the cursor and the filter.
pub struct Panel {
    root: PathBuf,
    items: Vec<Item>,
    rows: Vec<Row>,
    /// The cursor and the scroll over `rows`; the cursor lands only on problems, not on file rows.
    list: ui::table::Table,
    filter: Filter,
    keymap: ui::keys::Keymap,
}

impl Panel {
    /// `root`: the workspace; file names are shown relative to it. Key prefixes start with `leader`.
    pub fn new(root: impl Into<PathBuf>, leader: &str) -> Panel {
        Panel {
            root: root.into(),
            items: Vec::new(),
            rows: Vec::new(),
            list: ui::table::Table::new(),
            filter: Filter::All,
            keymap: ui::keys::Keymap::new(leader, &keys::commands(), &[]),
        }
    }

    /// The pending key-prefix menu, for the host to draw.
    pub fn menu(&self) -> Option<ui::keys::Menu> {
        self.keymap.menu()
    }

    /// Every action with its keys, for the host's help screen.
    pub fn keys_rows(&self) -> Vec<ui::keys::Row> {
        self.keymap.rows()
    }

    /// Replace the list. A list equal to the current one is ignored, so hosts can
    /// call this every frame.
    pub fn set_items(&mut self, mut items: Vec<Item>) {
        items.sort_by(|a, b| {
            a.path
                .cmp(&b.path)
                .then(a.line.cmp(&b.line))
                .then(a.col.cmp(&b.col))
                .then(a.severity.cmp(&b.severity))
        });
        if items == self.items {
            return;
        }
        let keep = self.current().cloned();
        self.items = items;
        self.rebuild(keep);
    }

    /// (errors, warnings, total) over all items, whatever the filter.
    pub fn counts(&self) -> (usize, usize, usize) {
        let e = self.items.iter().filter(|i| i.severity == Severity::Error).count();
        let w = self.items.iter().filter(|i| i.severity == Severity::Warning).count();
        (e, w, self.items.len())
    }

    /// The problem under the cursor, or `None` when the list is empty.
    fn current(&self) -> Option<&Item> {
        match self.rows.get(self.list.cursor())? {
            Row::Item(i) => self.items.get(*i),
            Row::File(..) => None,
        }
    }

    /// Rebuilds the rows from the items and the filter, keeping the cursor on `keep` when it is still shown.
    fn rebuild(&mut self, keep: Option<Item>) {
        self.rows.clear();
        let mut last: Option<&Path> = None;
        for (i, it) in self.items.iter().enumerate() {
            if !self.filter.keeps(it.severity) {
                continue;
            }
            if last != Some(it.path.as_path()) {
                last = Some(it.path.as_path());
                self.rows.push(Row::File(it.path.clone(), 0));
            }
            if let Some(Row::File(_, n)) =
                self.rows.iter_mut().rev().find(|r| matches!(r, Row::File(..)))
            {
                *n += 1;
            }
            self.rows.push(Row::Item(i));
        }
        // Stay on the same problem if it is still there.
        let at = keep
            .and_then(|k| {
                self.rows.iter().position(|r| matches!(r, Row::Item(i) if self.items[*i] == k))
            })
            .unwrap_or(self.list.cursor());
        let rows = &self.rows;
        self.list.set_rows(rows.len(), |r| matches!(rows[r], Row::Item(_)));
        self.list.set_cursor(at);
    }

    /// Handles a key press and tells the host what to do (open a place, close, or nothing).
    pub fn handle_event(&mut self, ev: &Event) -> Action {
        let Some(key) = ui::keys::pressed(ev) else { return Action::None };
        match self.keymap.feed(key) {
            ui::keys::Feed::Run(id) => self.run(id),
            _ => Action::None,
        }
    }

    /// Runs one action by id. Also lets the host run an action picked from its palette.
    pub fn run(&mut self, id: &'static str) -> Action {
        if ui::app::run_nav(&mut self.list, id) {
            return Action::None;
        }
        match id {
            "item.open" => {
                if let Some(it) = self.current() {
                    return Action::Open { path: it.path.clone(), line: it.line, col: it.col };
                }
            }
            "view.severity" => {
                let keep = self.current().cloned();
                self.filter = self.filter.next();
                self.rebuild(keep);
            }
            "palette" => return Action::Palette,
            "help" => return Action::Help,
            "quit" => return Action::Close,
            _ => {}
        }
        Action::None
    }

    /// The theme colour that marks a severity.
    fn sev_color(theme: &Theme, s: Severity) -> Color {
        let c = &theme.colors;
        match s {
            Severity::Error => c.error,
            Severity::Warning => c.warning,
            Severity::Info => c.info,
            Severity::Hint => c.muted,
        }
        .into()
    }

    /// Draws the list into `area`, scrolling to keep the cursor in view.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, cx: &ui::Context, focused: bool) {
        let theme = cx.theme;
        let c = &theme.colors;
        let (e, w, _) = self.counts();
        let plural = |n: usize, s: &str| format!("{n} {s}{}", if n == 1 { "" } else { "s" });
        let title = if self.items.is_empty() {
            "diagnostics".to_string()
        } else {
            format!(
                "diagnostics: {}, {}{}",
                plural(e, "error"),
                plural(w, "warning"),
                self.filter.label()
            )
        };
        let pane = Pane::new().title(&title).icon(theme.icons.diagnostics).focused(focused);
        let inner = pane.draw(cx, area, buf);
        if inner.height == 0 || inner.width < 6 {
            return;
        }
        let empty =
            if self.items.is_empty() { "no problems" } else { "nothing at this filter (f)" };
        let (rows, items, root) = (&self.rows, &self.items, &self.root);
        let row = |i: usize| match &rows[i] {
            Row::File(path, n) => ui::table::Row::new(vec![
                ui::table::Cell::new(vec![
                    Span::styled(theme.icons.fold_open.glyph, Style::new().fg(c.muted.into())),
                    Span::styled(
                        // Relative to the workspace root, for a shorter heading.
                        path.strip_prefix(root).unwrap_or(path).display().to_string(),
                        Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("  {n}"), Style::new().fg(c.muted.into())),
                ])
                .shrinking(1),
            ]),
            Row::Item(idx) => {
                let it = &items[*idx];
                // The first line of the message is what shrinks; the source at the right drops first.
                let message = ui::table::Cell::new(vec![
                    Span::raw("  "),
                    Span::styled("● ", Style::new().fg(Self::sev_color(theme, it.severity))),
                    Span::styled(
                        format!("{}:{}", it.line, it.col),
                        Style::new().fg(c.muted.into()),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        it.message.lines().next().unwrap_or("").to_string(),
                        Style::new().fg(c.fg.into()),
                    ),
                ])
                .shrinking(4);
                ui::table::Row {
                    id: ui::anim::hash_of(&(it.line, it.col, &it.message)),
                    cells: vec![message, it.source.clone().unwrap_or_default().into()],
                    ..Default::default()
                }
            }
        };
        let columns = [ui::table::Column::list(), ui::table::Column::trailing()];
        self.list
            .view(rows.len(), &columns, row)
            .pane(area)
            .focused(focused)
            .empty(empty)
            .render(cx, inner, buf);
    }
}

/// Tests for the panel.
#[cfg(test)]
mod tests;
