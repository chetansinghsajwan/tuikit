//! The symbol explorer: the symbols of the file being edited (functions, types, fields) as a tree you
//! can fold, filter and jump from, like the file explorer is for folders. The host (helix) supplies the
//! symbols, from a language server, and says which line the cursor is on; the panel draws them and
//! reports where to jump.

pub mod keys;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::collections::HashSet;
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;
use ui::{Pane, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Module,
    Class,
    Struct,
    Interface,
    Enum,
    Function,
    Method,
    Constructor,
    Field,
    Variable,
    Constant,
    TypeParameter,
    Other,
}

impl Kind {
    fn glyph(self) -> &'static str {
        match self {
            Kind::Module => "\u{f0169}",
            Kind::Class | Kind::Struct => "\u{f0831}",
            Kind::Interface => "\u{f0b6f}",
            Kind::Enum => "\u{f12a7}",
            Kind::Function | Kind::Method | Kind::Constructor => "\u{f0295}",
            Kind::Field => "\u{f0e69}",
            Kind::Variable => "\u{f0ae7}",
            Kind::Constant => "\u{f03ff}",
            Kind::TypeParameter => "\u{f0b68}",
            Kind::Other => "\u{f0a7a}",
        }
    }

    fn color(self, theme: &Theme) -> Color {
        let s = &theme.colors.syntax;
        match self {
            Kind::Module | Kind::Class | Kind::Struct | Kind::Interface | Kind::Enum | Kind::TypeParameter => s.type_.into(),
            Kind::Function | Kind::Method | Kind::Constructor => s.function.into(),
            Kind::Field | Kind::Variable => s.variable.into(),
            Kind::Constant => s.constant.into(),
            Kind::Other => theme.colors.muted.into(),
        }
    }
}

/// A symbol and the symbols inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    /// The type or signature, shown after the name.
    pub detail: String,
    pub kind: Kind,
    /// Where the name is: 0-based line and column.
    pub line: u32,
    pub col: u32,
    /// The last line of its body.
    pub end_line: u32,
    pub children: Vec<Symbol>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    /// Jump to this 0-based line and column.
    Goto { line: u32, col: u32 },
    /// Give the keyboard back to the editor.
    Close,
    /// The palette or the help were asked for (hosts that draw them handle these).
    Palette,
    Help,
}

/// One drawn row: where the symbol is in the tree (`path`) and what to show.
struct Row {
    path: Vec<usize>,
    depth: usize,
    name: String,
    detail: String,
    kind: Kind,
    line: u32,
    col: u32,
    end_line: u32,
    folded: Option<bool>,
}

enum Mode {
    Normal,
    Filter(Input),
}

pub struct Panel {
    symbols: Vec<Symbol>,
    rows: Vec<Row>,
    cursor: usize,
    offset: usize,
    page: usize,
    /// Symbols with members that are folded, by their position in the tree.
    folded: HashSet<Vec<usize>>,
    filter: String,
    mode: Mode,
    /// The cursor line of the editor: while the panel is not focused, the cursor follows it.
    following: Option<u32>,
    keymap: ui::keys::Keymap,
    message: Option<String>,
}

impl Default for Panel {
    fn default() -> Self {
        Panel::new("space")
    }
}

impl Panel {
    pub fn new(leader: &str) -> Panel {
        Panel {
            symbols: Vec::new(),
            rows: Vec::new(),
            cursor: 0,
            offset: 0,
            page: 10,
            folded: HashSet::new(),
            filter: String::new(),
            mode: Mode::Normal,
            following: None,
            keymap: ui::keys::Keymap::new(leader, keys::ACTIONS, keys::GROUPS),
            message: None,
        }
    }

    /// New symbols for the file (an empty list when there are none).
    pub fn set_symbols(&mut self, symbols: Vec<Symbol>) {
        if self.symbols != symbols {
            self.symbols = symbols;
            self.rebuild();
        }
    }

    /// The line the editor's cursor is on (0-based); while the panel has no focus it follows.
    pub fn follow(&mut self, line: Option<u32>) {
        self.following = line;
    }

    pub fn is_typing(&self) -> bool {
        matches!(self.mode, Mode::Filter(_))
    }

    pub fn menu(&self) -> Option<ui::keys::Menu> {
        self.keymap.menu()
    }

    pub fn keys_rows(&self) -> Vec<ui::keys::Row> {
        self.keymap.rows()
    }

    fn rebuild(&mut self) {
        let keep = self.rows.get(self.cursor).map(|r| r.path.clone());
        let q = self.filter.to_lowercase();
        let mut rows = Vec::new();
        fn walk(list: &[Symbol], path: &mut Vec<usize>, depth: usize, folded: &HashSet<Vec<usize>>, q: &str, rows: &mut Vec<Row>) -> bool {
            let mut any = false;
            for (i, s) in list.iter().enumerate() {
                path.push(i);
                let start = rows.len();
                let hit = q.is_empty() || s.name.to_lowercase().contains(q);
                let fold = !q.is_empty() || !folded.contains(path);
                rows.push(Row { path: path.clone(), depth, name: s.name.clone(), detail: s.detail.clone(), kind: s.kind, line: s.line, col: s.col, end_line: s.end_line, folded: (!s.children.is_empty()).then_some(!fold) });
                let inner = if fold { walk(&s.children, path, depth + 1, folded, q, rows) } else { false };
                // With a filter a symbol stays when its name or a member matches.
                if hit || inner {
                    any = true;
                } else {
                    rows.truncate(start);
                }
                path.pop();
            }
            any
        }
        walk(&self.symbols, &mut Vec::new(), 0, &self.folded, &q, &mut rows);
        self.rows = rows;
        self.cursor = keep.and_then(|p| self.rows.iter().position(|r| r.path == p)).unwrap_or(self.cursor).min(self.rows.len().saturating_sub(1));
    }

    /// The row of the innermost symbol that contains `line`.
    fn row_at_line(&self, line: u32) -> Option<usize> {
        self.rows.iter().enumerate().filter(|(_, r)| r.line <= line && line <= r.end_line.max(r.line)).max_by_key(|(_, r)| (r.depth, r.line)).map(|(i, _)| i)
    }

    fn set_fold(&mut self, path: Vec<usize>, fold: bool) {
        if fold {
            self.folded.insert(path);
        } else {
            self.folded.remove(&path);
        }
        self.rebuild();
    }

    fn fold_all(&mut self, fold: bool) {
        self.folded.clear();
        if fold {
            fn collect(list: &[Symbol], path: &mut Vec<usize>, out: &mut HashSet<Vec<usize>>) {
                for (i, s) in list.iter().enumerate() {
                    path.push(i);
                    if !s.children.is_empty() {
                        out.insert(path.clone());
                        collect(&s.children, path, out);
                    }
                    path.pop();
                }
            }
            let mut all = HashSet::new();
            collect(&self.symbols, &mut Vec::new(), &mut all);
            self.folded = all;
        }
        self.rebuild();
    }

    fn move_by(&mut self, delta: isize) {
        let last = self.rows.len().saturating_sub(1) as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, last.max(0)) as usize;
    }

    pub fn handle_event(&mut self, ev: &Event) -> Action {
        let Event::Key(key @ KeyEvent { kind, .. }) = ev else { return Action::None };
        if *kind == KeyEventKind::Release {
            return Action::None;
        }
        if let Mode::Filter(mut input) = std::mem::replace(&mut self.mode, Mode::Normal) {
            match key.code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.rebuild();
                }
                KeyCode::Enter => {}
                _ => {
                    input.handle_event(ev);
                    self.filter = input.value().to_string();
                    self.rebuild();
                    self.mode = Mode::Filter(input);
                }
            }
            return Action::None;
        }
        self.message = None;
        match self.keymap.feed(key) {
            ui::keys::Feed::Run(id) => self.run(id),
            ui::keys::Feed::NoBinding => {
                self.message = Some(format!("{} is not bound", self.keymap.rejected()));
                Action::None
            }
            ui::keys::Feed::Pending | ui::keys::Feed::Cancelled | ui::keys::Feed::Unbound => Action::None,
        }
    }

    pub fn run(&mut self, id: &'static str) -> Action {
        match id {
            "nav.up" => self.move_by(-1),
            "nav.down" => self.move_by(1),
            "nav.up5" => self.move_by(-5),
            "nav.down5" => self.move_by(5),
            "nav.page_up" => self.move_by(-(self.page as isize)),
            "nav.page_down" => self.move_by(self.page as isize),
            "nav.top" => self.cursor = 0,
            "nav.bottom" => self.cursor = self.rows.len().saturating_sub(1),
            "tree.out" => {
                if let Some(r) = self.rows.get(self.cursor) {
                    if r.folded == Some(false) {
                        let p = r.path.clone();
                        self.set_fold(p, true);
                    } else if r.path.len() > 1 {
                        let parent = r.path[..r.path.len() - 1].to_vec();
                        if let Some(i) = self.rows.iter().position(|x| x.path == parent) {
                            self.cursor = i;
                        }
                    }
                }
            }
            "tree.in" => {
                if let Some(r) = self.rows.get(self.cursor) {
                    if r.folded == Some(true) {
                        let p = r.path.clone();
                        self.set_fold(p, false);
                    }
                }
            }
            "item.open" => {
                if let Some(r) = self.rows.get(self.cursor) {
                    return Action::Goto { line: r.line, col: r.col };
                }
            }
            "tree.fold_all" => self.fold_all(true),
            "tree.unfold_all" => self.fold_all(false),
            "filter.start" => self.mode = Mode::Filter(Input::new(self.filter.clone())),
            "palette" => return Action::Palette,
            "help" => return Action::Help,
            "quit" => {
                if self.filter.is_empty() {
                    return Action::Close;
                }
                self.filter.clear();
                self.rebuild();
            }
            _ => {}
        }
        Action::None
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme, focused: bool) {
        let c = &theme.colors;
        let pane = Pane::new(theme).title("Symbols").icon(ui::icons::BRANCH).focused(focused);
        let mut inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height == 0 || inner.width < 6 {
            return;
        }
        // The filter prompt or a message takes the last row.
        let footer = match &self.mode {
            Mode::Filter(input) => Some((ui::prompt::line(theme, "filter: ", input.value()), Some(input.visual_cursor()))),
            Mode::Normal if !self.filter.is_empty() => Some((Line::from(Span::styled(format!("filter: {}", self.filter), Style::new().fg(c.warning.into()))), None)),
            Mode::Normal => self.message.as_ref().map(|m| (Line::from(Span::styled(m.clone(), Style::new().fg(c.error.into()))), None)),
        };
        if let Some((line, cursor)) = footer {
            let row = Rect { y: inner.bottom() - 1, height: 1, ..inner };
            Paragraph::new(line).render(row, buf);
            if let Some(cur) = cursor {
                ui::prompt::cursor(buf, row, "filter: ", cur);
            }
            inner.height -= 1;
            if inner.height == 0 {
                return;
            }
        }
        if self.rows.is_empty() {
            let why = if self.filter.is_empty() { "no symbols (no language server?)" } else { "nothing matches" };
            buf.set_string(inner.x + 1, inner.y, why, Style::new().fg(c.muted.into()));
            return;
        }
        // Without the focus the cursor stays on the symbol the editor's cursor is in.
        if !focused && !self.is_typing() {
            if let Some(i) = self.following.and_then(|l| self.row_at_line(l)) {
                self.cursor = i;
            }
        }
        self.page = usize::from(inner.height).max(1);
        if self.cursor < self.offset {
            self.offset = self.cursor;
        } else if self.cursor >= self.offset + self.page {
            self.offset = self.cursor + 1 - self.page;
        }
        let geo = ui::list::Rows::new(area, inner);
        for (n, row) in self.rows.iter().enumerate().skip(self.offset).take(self.page) {
            let y = inner.y + (n - self.offset) as u16;
            let at = n == self.cursor;
            let base = if at && !focused { ui::list::active(theme) } else { ui::list::style(theme, at && focused, None) };
            buf.set_style(geo.bar(y), base);
            let mut spans = vec![
                ui::tree::guides(theme, row.depth),
                ui::tree::fold_mark(theme, row.folded),
                Span::styled(format!("{} ", row.kind.glyph()), Style::new().fg(row.kind.color(theme))),
                Span::styled(row.name.clone(), Style::new().fg(c.fg.into()).add_modifier(if row.folded.is_some() { Modifier::BOLD } else { Modifier::empty() })),
            ];
            if !row.detail.is_empty() {
                spans.push(Span::styled(format!(" {}", row.detail), Style::new().fg(c.muted.into())));
            }
            Paragraph::new(Line::from(spans)).style(base).render(geo.content(y), buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn sym(name: &str, kind: Kind, line: u32, end: u32, children: Vec<Symbol>) -> Symbol {
        Symbol { name: name.into(), detail: String::new(), kind, line, col: 4, end_line: end, children }
    }

    fn panel() -> Panel {
        let mut p = Panel::default();
        p.set_symbols(vec![
            sym("Config", Kind::Struct, 0, 9, vec![sym("name", Kind::Field, 1, 1, vec![]), sym("size", Kind::Field, 2, 2, vec![])]),
            sym("load", Kind::Function, 11, 20, vec![]),
        ]);
        p
    }

    fn key(c: KeyCode) -> Event {
        Event::Key(KeyEvent::new(c, KeyModifiers::NONE))
    }

    fn press(p: &mut Panel, s: &str) -> Action {
        let mut last = Action::None;
        for c in s.chars() {
            last = p.handle_event(&key(KeyCode::Char(c)));
        }
        last
    }

    fn draw(p: &mut Panel, focused: bool) -> String {
        let area = Rect::new(0, 0, 40, 10);
        let mut b = Buffer::empty(area);
        p.render(area, &mut b, &Theme::default(), focused);
        (0..10).map(|y| (0..40).map(|x| b[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    }

    #[test]
    fn the_tree_folds_unfolds_and_enter_jumps() {
        let mut p = panel();
        let text = draw(&mut p, true);
        assert!(text.contains("Config") && text.contains("name") && text.contains("load"), "{text}");
        // h folds the struct, l unfolds it.
        press(&mut p, "h");
        assert!(!draw(&mut p, true).contains("name"));
        press(&mut p, "l");
        assert!(draw(&mut p, true).contains("name"));
        // Enter on a field goes to its line and column.
        press(&mut p, "j");
        assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::Goto { line: 1, col: 4 });
        // h on a member goes to the parent.
        press(&mut p, "h");
        assert_eq!(p.rows[p.cursor].name, "Config");
    }

    #[test]
    fn filtering_keeps_matches_and_their_parents_and_esc_clears() {
        let mut p = panel();
        press(&mut p, "/");
        press(&mut p, "siz");
        let text = draw(&mut p, true);
        assert!(text.contains("Config") && text.contains("size") && !text.contains("load") && !text.contains("name"), "{text}");
        p.handle_event(&key(KeyCode::Enter));
        assert!(draw(&mut p, true).contains("filter: siz"));
        assert_eq!(p.handle_event(&key(KeyCode::Esc)), Action::None, "the first Esc clears the filter");
        assert!(draw(&mut p, true).contains("load"));
        assert_eq!(p.handle_event(&key(KeyCode::Esc)), Action::Close);
    }

    #[test]
    fn without_the_focus_the_cursor_follows_the_editor_line() {
        let mut p = panel();
        p.follow(Some(2));
        draw(&mut p, false);
        assert_eq!(p.rows[p.cursor].name, "size");
        p.follow(Some(15));
        draw(&mut p, false);
        assert_eq!(p.rows[p.cursor].name, "load");
        p.follow(Some(5));
        draw(&mut p, false);
        assert_eq!(p.rows[p.cursor].name, "Config", "the innermost symbol that contains the line");
        assert!(draw(&mut Panel::default(), true).contains("no symbols"));
    }
}
