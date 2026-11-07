//! `diagnostics-panel`: the problems list (errors, warnings, ...) grouped by
//! file, with a severity filter. It knows nothing about any editor: the host
//! hands it a list of `Item`s (it can do so on every frame; an unchanged list
//! costs a comparison) and reacts to the `Action`s.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::path::{Path, PathBuf};
use ui::{Pane, Theme};
use unicode_width::UnicodeWidthStr;

/// Most severe first (`Error < Warning < ...` in sort order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    /// Errors and warnings.
    Problems,
    Errors,
}

impl Filter {
    fn next(self) -> Filter {
        match self {
            Filter::All => Filter::Problems,
            Filter::Problems => Filter::Errors,
            Filter::Errors => Filter::All,
        }
    }

    fn keeps(self, s: Severity) -> bool {
        match self {
            Filter::All => true,
            Filter::Problems => s <= Severity::Warning,
            Filter::Errors => s == Severity::Error,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Filter::All => "",
            Filter::Problems => " [errors + warnings]",
            Filter::Errors => " [errors]",
        }
    }
}

enum Row {
    /// A file: the number of problems shown under it.
    File(PathBuf, usize),
    /// Index into `items`.
    Item(usize),
}

pub struct Panel {
    root: PathBuf,
    items: Vec<Item>,
    rows: Vec<Row>,
    /// Indices into `rows` of the selectable rows.
    selectable: Vec<usize>,
    /// Position in `selectable`.
    cursor: usize,
    offset: usize,
    page: usize,
    filter: Filter,
}

impl Panel {
    /// `root`: the workspace; file names are shown relative to it.
    pub fn new(root: impl Into<PathBuf>) -> Panel {
        Panel {
            root: root.into(),
            items: Vec::new(),
            rows: Vec::new(),
            selectable: Vec::new(),
            cursor: 0,
            offset: 0,
            page: 10,
            filter: Filter::All,
        }
    }

    /// Replace the list. A list equal to the current one is ignored, so hosts can
    /// call this every frame.
    pub fn set_items(&mut self, mut items: Vec<Item>) {
        items.sort_by(|a, b| {
            a.path.cmp(&b.path).then(a.line.cmp(&b.line)).then(a.col.cmp(&b.col)).then(a.severity.cmp(&b.severity))
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

    fn current(&self) -> Option<&Item> {
        match self.rows.get(*self.selectable.get(self.cursor)?)? {
            Row::Item(i) => self.items.get(*i),
            Row::File(..) => None,
        }
    }

    fn rebuild(&mut self, keep: Option<Item>) {
        self.rows.clear();
        self.selectable.clear();
        let mut last: Option<&Path> = None;
        for (i, it) in self.items.iter().enumerate() {
            if !self.filter.keeps(it.severity) {
                continue;
            }
            if last != Some(it.path.as_path()) {
                last = Some(it.path.as_path());
                self.rows.push(Row::File(it.path.clone(), 0));
            }
            if let Some(Row::File(_, n)) = self.rows.iter_mut().rev().find(|r| matches!(r, Row::File(..))) {
                *n += 1;
            }
            self.selectable.push(self.rows.len());
            self.rows.push(Row::Item(i));
        }
        // Stay on the same problem if it is still there.
        self.cursor = keep
            .and_then(|k| {
                self.selectable.iter().position(|&r| matches!(&self.rows[r], Row::Item(i) if self.items[*i] == k))
            })
            .unwrap_or(self.cursor)
            .min(self.selectable.len().saturating_sub(1));
    }

    fn move_by(&mut self, delta: isize) {
        let last = self.selectable.len().saturating_sub(1) as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, last.max(0)) as usize;
    }

    pub fn handle_event(&mut self, ev: &Event) -> Action {
        let Event::Key(KeyEvent { code, modifiers, kind, .. }) = ev else { return Action::None };
        if *kind == KeyEventKind::Release {
            return Action::None;
        }
        let ctrl = modifiers.contains(KeyModifiers::CONTROL);
        match code {
            KeyCode::Esc | KeyCode::Char('q') => return Action::Close,
            KeyCode::Char('c') if ctrl => return Action::Close,
            KeyCode::Char('k') if ctrl => self.move_by(-5),
            KeyCode::Char('j') if ctrl => self.move_by(5),
            KeyCode::Up | KeyCode::Char('k') => self.move_by(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_by(1),
            KeyCode::Char('p') if ctrl => self.move_by(-1),
            KeyCode::Char('n') if ctrl => self.move_by(1),
            KeyCode::PageUp => self.move_by(-(self.page as isize)),
            KeyCode::PageDown => self.move_by(self.page as isize),
            KeyCode::Home | KeyCode::Char('g') => self.cursor = 0,
            KeyCode::End | KeyCode::Char('G') => self.cursor = self.selectable.len().saturating_sub(1),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                if let Some(it) = self.current() {
                    return Action::Open { path: it.path.clone(), line: it.line, col: it.col };
                }
            }
            KeyCode::Char('f') => {
                let keep = self.current().cloned();
                self.filter = self.filter.next();
                self.rebuild(keep);
            }
            _ => {}
        }
        Action::None
    }

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

    fn rel<'a>(&'a self, p: &'a Path) -> &'a Path {
        p.strip_prefix(&self.root).unwrap_or(p)
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme, focused: bool) {
        let c = &theme.colors;
        let (e, w, _) = self.counts();
        let plural = |n: usize, s: &str| format!("{n} {s}{}", if n == 1 { "" } else { "s" });
        let title = if self.items.is_empty() {
            "diagnostics".to_string()
        } else {
            format!("diagnostics: {}, {}{}", plural(e, "error"), plural(w, "warning"), self.filter.label())
        };
        let pane = Pane::new(theme).title(&title).icon(ui::icons::DIAGNOSTICS).focused(focused);
        let inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height == 0 || inner.width < 6 {
            return;
        }
        if self.rows.is_empty() {
            let msg = if self.items.is_empty() { "no problems" } else { "nothing at this filter (f)" };
            buf.set_string(inner.x + 1, inner.y, msg, Style::new().fg(c.muted.into()));
            return;
        }

        self.page = usize::from(inner.height).max(1);
        let crow = self.selectable.get(self.cursor).copied().unwrap_or(0);
        if crow < self.offset {
            self.offset = crow;
        } else if crow >= self.offset + self.page {
            self.offset = crow + 1 - self.page;
        }
        let geo = ui::list::Rows::new(area, inner);
        for (i, row) in self.rows.iter().enumerate().skip(self.offset).take(self.page) {
            let y = inner.y + (i - self.offset) as u16;
            let line_area = Rect { y, height: 1, ..inner };
            match row {
                Row::File(path, n) => {
                    let line = Line::from(vec![
                        Span::styled("▾ ", Style::new().fg(c.muted.into())),
                        Span::styled(
                            self.rel(path).display().to_string(),
                            Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!("  {n}"), Style::new().fg(c.muted.into())),
                    ]);
                    Paragraph::new(line).render(line_area, buf);
                }
                Row::Item(idx) => {
                    let it = &self.items[*idx];
                    let selected = i == crow;
                    let head = format!("{}:{}", it.line, it.col);
                    let mut source = it.source.as_deref().map(|s| format!("  {s}")).unwrap_or_default();
                    // The first line of the message, cut to the room left.
                    let fixed = 2 + 2 + head.width() + 2;
                    // The source goes when it would leave the message under 4 cells.
                    if usize::from(inner.width).saturating_sub(fixed + source.width()) < 4 {
                        source.clear();
                    }
                    let room = usize::from(inner.width).saturating_sub(fixed + source.width());
                    let mut msg: String = it.message.lines().next().unwrap_or("").to_string();
                    if msg.width() > room {
                        let mut cut = String::new();
                        for ch in msg.chars() {
                            if cut.width() + 1 >= room {
                                break;
                            }
                            cut.push(ch);
                        }
                        msg = format!("{cut}…");
                    }
                    let line = Line::from(vec![
                        Span::styled(if selected { "▸ " } else { "  " }, Style::new().fg(c.accent.into())),
                        Span::styled("● ", Style::new().fg(Self::sev_color(theme, it.severity))),
                        Span::styled(head, Style::new().fg(c.muted.into())),
                        Span::raw("  "),
                        Span::styled(msg, Style::new().fg(c.fg.into())),
                        Span::styled(source, Style::new().fg(c.muted.into())),
                    ]);
                    ui::list::ListRow {
                        id: ui::anim::hash_of(&(it.line, it.col, &it.message)),
                        left: line.spans,
                        selected,
                        focused,
                        ..Default::default()
                    }
                    .draw(theme, buf, &geo, y);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
