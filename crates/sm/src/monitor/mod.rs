//! The monitor screen: the CPU, memory, network and process panes, and the state that is the
//! user's (cursor, sort, filter, tree, a signal waiting for a yes).

use crate::format;
use crate::procs::{self, Options, Row, Sort};
use crate::sample::{Sampler, ratio};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use sysinfo::Signal;
use tui_input::Input;
use ui::app::{Host, Navigable, Request, Screen};
use ui::footer::{Footer, Notice};
use ui::list::{ListRow, Rows};
use ui::prompt::Typed;
use ui::{Pane, Theme, graph};

/// What the footer is busy with: nothing, a filter being typed, or a signal waiting for a yes.
enum Mode {
    Normal,
    Filter(Input),
    Confirm { pid: u32, name: String, signal: Signal },
}

/// A labeled amount used out of a total, drawn as two rows: the numbers, then a bar.
struct Gauge<'a> {
    label: &'a str,
    used: u64,
    total: u64,
}

impl<'a> Gauge<'a> {
    fn new(label: &'a str, used: u64, total: u64) -> Self {
        Gauge { label, used, total }
    }
}

/// The monitor: readings plus what the user chose to see.
pub struct Monitor {
    sampler: Sampler,
    options: Options,
    mode: Mode,
    message: Option<Notice>,
    cursor: usize,
    top: usize,
    page: usize,
    /// The process under the cursor, so a refresh that reorders the list keeps the cursor on it.
    selected: Option<u32>,
}

impl Monitor {
    /// Opens the monitor with its first reading.
    pub fn new() -> Result<Monitor, String> {
        Ok(Monitor {
            sampler: Sampler::new()?,
            options: Options {
                sort: Sort::Cpu,
                is_reversed: false,
                filter: String::new(),
                is_tree: false,
            },
            mode: Mode::Normal,
            message: None,
            cursor: 0,
            top: 0,
            page: 1,
            selected: None,
        })
    }

    /// The rows of the process list now.
    fn rows(&self) -> Vec<Row> {
        procs::rows(&self.sampler.snapshot.procs, &self.options)
    }

    /// Moves the cursor back onto the process it was on after the list changed; when that process
    /// is gone the cursor keeps its place.
    fn follow_selected(&mut self) {
        let rows = self.rows();
        if let Some(pid) = self.selected
            && let Some(at) =
                rows.iter().position(|r| self.sampler.snapshot.procs[r.index].pid == pid)
        {
            self.cursor = at;
        }
        self.cursor = self.cursor.min(rows.len().saturating_sub(1));
        self.remember(&rows);
    }

    /// Notes which process the cursor is on.
    fn remember(&mut self, rows: &[Row]) {
        self.selected = rows.get(self.cursor).map(|r| self.sampler.snapshot.procs[r.index].pid);
    }

    /// Shows a message until the next key press.
    fn say(&mut self, text: impl Into<String>, is_error: bool) {
        self.message = Some(Notice::new(text, is_error));
    }

    /// Asks before sending `signal` to the process under the cursor.
    fn ask_signal(&mut self, signal: Signal) {
        let rows = self.rows();
        let Some(row) = rows.get(self.cursor) else { return };
        let p = &self.sampler.snapshot.procs[row.index];
        self.mode = Mode::Confirm { pid: p.pid, name: p.name.clone(), signal };
    }

    /// The color of a load: calm, busy or critical.
    fn load_color(theme: &Theme, ratio: f32) -> Color {
        let c = &theme.colors;
        if ratio < 0.5 {
            c.success.into()
        } else if ratio < 0.8 {
            c.warning.into()
        } else {
            c.error.into()
        }
    }

    /// A pane titled `title` over `area`, returning its inner area (the pane is drawn).
    fn pane(theme: &Theme, buf: &mut Buffer, area: Rect, title: &str, focused: bool) -> Rect {
        let mut pane = Pane::new(theme).title(title).focused(focused);
        if let Some(icon) = theme.icons.title(title) {
            pane = pane.icon(icon);
        }
        let inner = pane.inner(area);
        pane.render(area, buf);
        inner
    }

    /// The CPU pane: the history graph, then the cores in two columns.
    fn draw_cpu(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let s = &self.sampler.snapshot;
        let title = format!(
            "cpu {:.0}%  load {:.2} {:.2} {:.2}  up {}",
            s.cpu,
            s.load.0,
            s.load.1,
            s.load.2,
            format::duration(s.uptime)
        );
        let inner = Self::pane(theme, buf, area, &title, false);
        if inner.height == 0 || inner.width == 0 {
            return;
        }
        let core_rows = (s.cores.len() as u16).div_ceil(2).min(inner.height / 2);
        let graph_area = Rect { height: inner.height - core_rows, ..inner };
        let color = Self::load_color(theme, s.cpu / 100.0);
        graph::history(buf, graph_area, &self.sampler.cpu_history.values(), color);
        let half = inner.width / 2;
        for (i, load) in s.cores.iter().enumerate() {
            let row = i as u16 % core_rows.max(1);
            let column = i as u16 / core_rows.max(1);
            if column > 1 {
                break;
            }
            let cell = Rect {
                x: inner.x + column * half,
                y: graph_area.bottom() + row,
                width: if column == 0 { half } else { inner.width - half },
                height: 1,
            };
            self.draw_core(cell, buf, theme, i, *load);
        }
    }

    /// One core: its number, a meter and its percent.
    fn draw_core(&self, area: Rect, buf: &mut Buffer, theme: &Theme, i: usize, load: f32) {
        let label = format!("{i:>2} ");
        let value = format!(" {load:>3.0}%");
        let used = (label.len() + value.len() + 1) as u16;
        if area.width <= used {
            return;
        }
        let muted = Style::new().fg(theme.colors.muted.into());
        buf.set_string(area.x, area.y, &label, muted);
        let meter = Rect { x: area.x + label.len() as u16, width: area.width - used, ..area };
        let color = Self::load_color(theme, load / 100.0);
        graph::meter(buf, meter, load / 100.0, color, theme.colors.border.into());
        buf.set_string(meter.right(), area.y, &value, Style::new().fg(color));
    }

    /// One labeled meter over two rows (the label with its numbers, then the bar); returns the next row.
    fn draw_meter(area: Rect, buf: &mut Buffer, theme: &Theme, y: u16, gauge: Gauge) -> u16 {
        let Gauge { label, used, total } = gauge;
        if y + 1 >= area.bottom() {
            return area.bottom();
        }
        let r = ratio(used, total);
        let color = Self::load_color(theme, r);
        let line = Line::from(vec![
            Span::raw(label.to_string()),
            Span::styled(
                format!("  {} / {}  {:.0}%", format::bytes(used), format::bytes(total), r * 100.0),
                Style::new().fg(theme.colors.muted.into()),
            ),
        ]);
        let row = |y| Rect { y, height: 1, ..area };
        Paragraph::new(line).render(row(y), buf);
        graph::meter(buf, row(y + 1), r, color, theme.colors.border.into());
        y + 2
    }

    /// The memory pane: memory, swap and every disk.
    fn draw_memory(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let s = &self.sampler.snapshot;
        let inner = Self::pane(theme, buf, area, "memory", false);
        if inner.height == 0 || inner.width == 0 {
            return;
        }
        let mut y = inner.y;
        y = Self::draw_meter(
            inner,
            buf,
            theme,
            y,
            Gauge::new("mem", s.memory_used, s.memory_total),
        );
        if s.swap_total > 0 {
            y = Self::draw_meter(
                inner,
                buf,
                theme,
                y,
                Gauge::new("swap", s.swap_used, s.swap_total),
            );
        }
        for disk in &s.disks {
            y = Self::draw_meter(
                inner,
                buf,
                theme,
                y,
                Gauge::new(&disk.mount, disk.used, disk.total),
            );
        }
    }

    /// The network pane: download over upload, each a label and a history graph.
    fn draw_network(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let s = &self.sampler.snapshot;
        let inner = Self::pane(theme, buf, area, "network", false);
        if inner.height < 2 || inner.width == 0 {
            return;
        }
        let half = inner.height / 2;
        let parts = [
            ("▼", s.download, &self.sampler.download_history, theme.colors.info),
            ("▲", s.upload, &self.sampler.upload_history, theme.colors.accent),
        ];
        for (n, (sign, now, history, color)) in parts.into_iter().enumerate() {
            let top = inner.y + n as u16 * half;
            let height = if n == 0 { half } else { inner.bottom() - top };
            let label = format!("{sign} {}", format::rate(now));
            buf.set_string(inner.x, top, label, Style::new().fg(color.into()));
            let graph_area = Rect { y: top + 1, height: height - 1, ..inner };
            graph::history(buf, graph_area, &history.scaled(4096.0), color.into());
        }
    }

    /// The process pane: a header, then the rows with the cursor.
    fn draw_procs(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let rows = self.rows();
        let title = format!(
            "processes {}  by {}{}{}",
            rows.len(),
            self.options.sort.label(),
            if self.options.is_reversed { " reversed" } else { "" },
            if self.options.filter.is_empty() {
                String::new()
            } else {
                format!("  filter {}", self.options.filter)
            }
        );
        let pane = Pane::new(theme).title(&title).focused(true);
        let inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height < 2 {
            return;
        }
        let muted = Style::new().fg(theme.colors.muted.into());
        let header = Rect { height: 1, ..inner };
        Paragraph::new(Line::from(Span::styled("    PID  NAME", muted))).render(header, buf);
        let right = " CPU%     MEM STATE";
        let x = inner.right().saturating_sub(right.len() as u16);
        buf.set_string(x.max(inner.x), inner.y, right, muted);

        let list = Rect { y: inner.y + 1, height: inner.height - 1, ..inner };
        self.page = usize::from(list.height).max(1);
        self.top = ui::list::follow_cursor(self.cursor, self.top, self.page, rows.len());
        let geometry = Rows::new(area, list);
        let procs = &self.sampler.snapshot.procs;
        for (i, row) in rows.iter().skip(self.top).take(self.page).enumerate() {
            let p = &procs[row.index];
            let indent = "  ".repeat(row.depth);
            let cpu_color = Self::load_color(theme, p.cpu / 100.0);
            ListRow {
                left: vec![
                    Span::styled(format!("{:>7}  ", p.pid), muted),
                    Span::raw(format!("{indent}{}", p.name)),
                ],
                right: vec![
                    Span::styled(format!("{:>5.1}", p.cpu), Style::new().fg(cpu_color)),
                    Span::styled(format!(" {:>7} {:<5}", format::bytes(p.memory), p.status), muted),
                ],
                min_left: 20,
                is_selected: self.top + i == self.cursor,
                is_focused: true,
                ..ListRow::default()
            }
            .draw(theme, buf, &geometry, list.y + i as u16);
        }
    }

    /// The footer line: a prompt, a question, a message, or the hints.
    fn draw_footer(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let footer = match &self.mode {
            Mode::Filter(i) => Footer::Prompt {
                label: "filter: ",
                text: i.value().into(),
                cursor: i.visual_cursor(),
            },
            Mode::Confirm { pid, name, signal } => Footer::Question(format!(
                "Send {} to {name} ({pid})? y confirms",
                if *signal == Signal::Kill { "SIGKILL" } else { "SIGTERM" }
            )),
            Mode::Normal => match &self.message {
                Some(notice) => Footer::Notice(notice),
                None => Footer::Hint(
                    "s sort  r reverse  t tree  / filter  x terminate  X kill  ? help  q quit"
                        .into(),
                ),
            },
        };
        footer.render(area, buf, theme);
    }

    /// Feeds a key to the filter prompt or the question.
    fn typing(&mut self, ev: &Event) -> Request {
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Filter(mut input) => match ui::prompt::typed(&mut input, ev) {
                Typed::Submit(text) => {
                    self.options.filter = text;
                    self.cursor = 0;
                    self.top = 0;
                    self.follow_selected();
                }
                Typed::Cancel => {}
                Typed::Editing => {
                    self.options.filter = input.value().to_string();
                    self.cursor = 0;
                    self.mode = Mode::Filter(input);
                }
            },
            Mode::Confirm { pid, name, signal } => {
                let Some(key) = ui::keys::pressed(ev) else {
                    self.mode = Mode::Confirm { pid, name, signal };
                    return Request::None;
                };
                if matches!(key.code, KeyCode::Char('y' | 'Y') | KeyCode::Enter) {
                    if self.sampler.signal(pid, signal) {
                        self.say(format!("sent the signal to {name} ({pid})"), false);
                    } else {
                        self.say(format!("could not signal {name} ({pid})"), true);
                    }
                }
            }
            Mode::Normal => {}
        }
        Request::None
    }
}

/// Lets the shared app shell host the monitor.
impl Screen for Monitor {
    /// Draws the panes: cpu and memory and network on top, the processes below, the footer last.
    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.height < 6 || area.width < 20 {
            return;
        }
        let footer = Rect { y: area.bottom() - 1, height: 1, ..area };
        let body = Rect { height: area.height - 1, ..area };
        let top_height = (body.height * 2 / 5).clamp(6, 18).min(body.height - 3);
        let top = Rect { height: top_height, ..body };
        let bottom = Rect { y: top.bottom(), height: body.height - top_height, ..body };

        let left_width = top.width * 11 / 20;
        let cpu = Rect { width: left_width, ..top };
        let side = Rect { x: top.x + left_width, width: top.width - left_width, ..top };
        let memory_height = side.height * 3 / 5;
        let memory = Rect { height: memory_height, ..side };
        let network =
            Rect { y: side.y + memory_height, height: side.height - memory_height, ..side };

        self.draw_cpu(cpu, buf, theme);
        self.draw_memory(memory, buf, theme);
        self.draw_network(network, buf, theme);
        self.draw_procs(bottom, buf, theme);
        self.draw_footer(footer, buf, theme);
    }

    /// A filter or a question is open: the keys are theirs.
    fn captures_input(&self, _ev: &Event) -> bool {
        !matches!(self.mode, Mode::Normal)
    }

    fn on_event(&mut self, ev: &Event, _host: &mut dyn Host) -> Request {
        self.typing(ev)
    }

    fn run_action(&mut self, id: &'static str, _host: &mut dyn Host) -> Request {
        match id {
            "sort.next" => {
                self.options.sort = self.options.sort.next();
                self.follow_selected();
            }
            "sort.reverse" => {
                self.options.is_reversed = !self.options.is_reversed;
                self.follow_selected();
            }
            "view.tree" => {
                self.options.is_tree = !self.options.is_tree;
                self.follow_selected();
            }
            "filter.start" => self.mode = Mode::Filter(Input::new(self.options.filter.clone())),
            "filter.clear" => {
                self.options.filter.clear();
                self.follow_selected();
            }
            "proc.term" => self.ask_signal(Signal::Term),
            "proc.kill" => self.ask_signal(Signal::Kill),
            "quit" => return Request::Quit,
            _ => {}
        }
        Request::None
    }

    fn on_keypress(&mut self) {
        self.message = None;
    }

    /// Tells the user the key sequence they typed does nothing.
    fn on_unbound_keys(&mut self, keys: &str) {
        self.say(format!("{keys} is not bound"), true);
    }

    /// Reads the machine again; the screen is drawn again.
    fn on_tick(&mut self) -> bool {
        self.sampler.refresh();
        self.follow_selected();
        true
    }

    fn navigable(&mut self) -> Option<&mut dyn Navigable> {
        Some(self)
    }
}

/// The process rows are what the shared movement walks.
impl Navigable for Monitor {
    fn move_by(&mut self, delta: isize) {
        let rows = self.rows();
        self.cursor = ui::list::move_cursor(self.cursor, delta, rows.len());
        self.remember(&rows);
    }

    fn page_row_count(&self) -> usize {
        self.page
    }

    fn move_to_top(&mut self) {
        self.move_by(isize::MIN / 2);
    }

    fn move_to_bottom(&mut self) {
        self.move_by(isize::MAX / 2);
    }
}
