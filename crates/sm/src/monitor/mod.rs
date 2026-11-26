//! The monitor screen: the CPU, memory, network and process panes, and the state that is the
//! user's (cursor, sort, filter, tree, a signal waiting for a yes).

use crate::format;
use crate::panels::{Panel, Panels};
use crate::procs::{self, Options, Row, Sort};
use crate::sample::{Health, Sampler, Snapshot, ratio};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::time::{Duration, Instant};
use sysinfo::Signal;
use tui_input::Input;
use ui::app::{Host, Navigable, Request, Screen};
use ui::columns::{Column, Columns};
use ui::footer::{Footer, Notice};
use ui::graph::Eased;
use ui::list::{List, ListRow, Slot};
use ui::prompt::Typed;
use ui::{Component, Pane, graph, tree};

/// What the footer is busy with: nothing, a filter being typed, or a signal waiting for a yes.
enum Mode {
    Normal,
    Filter(Input),
    Confirm { pid: u32, name: String, signal: Signal },
}

/// The blank cells between the two columns of core meters.
const GAP: u16 = 2;
/// The cells a sensor's name takes before its bar; a longer name ends in `…`.
const LABEL_W: usize = 12;

/// A labeled amount used out of a total, drawn as two rows: the numbers, then a bar filled to
/// `shown` (0 to 1), which glides to the true ratio.
struct Gauge<'a> {
    label: &'a str,
    used: u64,
    total: u64,
    shown: f32,
    /// More about it after the numbers (a disk's reads, writes and health).
    notes: Vec<Span<'static>>,
}

impl<'a> Gauge<'a> {
    fn new(label: &'a str, used: u64, total: u64, shown: f32) -> Self {
        Gauge { label, used, total, shown, notes: Vec::new() }
    }
}

/// The values the bars show, each gliding to the latest reading instead of jumping.
#[derive(Default)]
struct Glide {
    cpu: Option<Eased>,
    cores: Vec<Eased>,
    memory: Option<Eased>,
    swap: Option<Eased>,
    disks: Vec<Eased>,
}

impl Glide {
    /// Sends every bar toward the readings of `s`.
    fn retarget(&mut self, s: &Snapshot, now: Instant) {
        let cores: Vec<f32> = s.cores.iter().map(|c| c / 100.0).collect();
        let disks: Vec<f32> = s.disks.iter().map(|d| ratio(d.used, d.total)).collect();
        Self::follow(&mut self.cores, &cores, now);
        Self::follow(&mut self.disks, &disks, now);
        let cpu = s.cpu / 100.0;
        self.cpu.get_or_insert(Eased::new(cpu)).set(cpu, now);
        let memory = ratio(s.memory_used, s.memory_total);
        self.memory.get_or_insert(Eased::new(memory)).set(memory, now);
        let swap = ratio(s.swap_used, s.swap_total);
        self.swap.get_or_insert(Eased::new(swap)).set(swap, now);
    }

    /// Makes `bars` as many as `targets` (new ones start at their target) and sets each target.
    fn follow(bars: &mut Vec<Eased>, targets: &[f32], now: Instant) {
        bars.truncate(targets.len());
        for (i, target) in targets.iter().enumerate() {
            match bars.get_mut(i) {
                Some(bar) => bar.set(*target, now),
                None => bars.push(Eased::new(*target)),
            }
        }
    }

    /// Whether any bar is still on its way.
    fn is_moving(&self, now: Instant) -> bool {
        self.cores
            .iter()
            .chain(&self.disks)
            .chain(self.cpu.iter())
            .chain(self.memory.iter())
            .chain(self.swap.iter())
            .any(|bar| bar.is_moving(now))
    }
}

/// The value of bar `i` of `bars` at `now`, or `fallback` when there is no such bar.
fn shown(bars: &[Eased], i: usize, now: Instant, fallback: f32) -> f32 {
    bars.get(i).map_or(fallback, |bar| bar.value(now))
}

/// The monitor: readings plus what the user chose to see.
pub struct Monitor {
    sampler: Sampler,
    /// How often the machine is read.
    interval: Duration,
    last_refresh: Instant,
    panels: Panels,
    glide: Glide,
    /// When the frame being drawn started, so every bar of a frame is at the same point of its glide.
    frame_time: Instant,
    options: Options,
    mode: Mode,
    message: Option<Notice>,
    /// The cursor and the scroll over the process rows.
    list: List,
    /// The process under the cursor, so a refresh that reorders the list keeps the cursor on it.
    selected: Option<u32>,
}

impl Monitor {
    /// Opens the monitor with its first reading, reading the machine every `interval`, with the
    /// `hidden` panels off.
    pub fn new(interval: Duration, hidden: &[Panel]) -> Result<Monitor, String> {
        let sampler = Sampler::new()?;
        let mut glide = Glide::default();
        glide.retarget(&sampler.snapshot, Instant::now());
        Ok(Monitor {
            sampler,
            interval,
            last_refresh: Instant::now(),
            panels: Panels::without(hidden),
            glide,
            frame_time: Instant::now(),
            options: Options {
                sort: Sort::Cpu,
                is_reversed: false,
                filter: String::new(),
                is_tree: false,
            },
            mode: Mode::Normal,
            message: None,
            list: List::new(),
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
            self.list.set_len(rows.len());
            self.list.set_cursor(at);
        }
        self.list.set_len(rows.len());
        self.remember(&rows);
    }

    /// Notes which process the cursor is on.
    fn remember(&mut self, rows: &[Row]) {
        self.selected =
            rows.get(self.list.cursor()).map(|r| self.sampler.snapshot.procs[r.index].pid);
    }

    /// Shows or hides `panel`; the last panel on stays on, and the message says so.
    fn toggle_panel(&mut self, panel: Panel) {
        if !self.panels.toggle(panel) {
            self.say(format!("{} is the last panel on", panel.label()), true);
        }
    }

    /// Shows a message until the next key press.
    fn say(&mut self, text: impl Into<String>, is_error: bool) {
        self.message = Some(Notice::new(text, is_error));
    }

    /// Asks before sending `signal` to the process under the cursor.
    fn ask_signal(&mut self, signal: Signal) {
        let rows = self.rows();
        let Some(row) = rows.get(self.list.cursor()) else { return };
        let p = &self.sampler.snapshot.procs[row.index];
        self.mode = Mode::Confirm { pid: p.pid, name: p.name.clone(), signal };
    }

    /// A pane titled `title` over `area`, returning its inner area (the pane is drawn).
    fn pane(cx: &ui::Context, buf: &mut Buffer, area: Rect, title: &str, focused: bool) -> Rect {
        let theme = cx.theme;
        let mut pane = Pane::new().title(title).focused(focused);
        if let Some(icon) = theme.icons.title(title) {
            pane = pane.icon(icon);
        }
        pane.draw(cx, area, buf)
    }

    /// The CPU pane: the load of the whole machine, then the cores in two columns, all as horizontal bars.
    fn draw_cpu(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let s = &self.sampler.snapshot;
        let title = format!(
            "cpu {:.0}%  load {:.2} {:.2} {:.2}  up {}",
            s.cpu,
            s.load.0,
            s.load.1,
            s.load.2,
            format::duration(s.uptime)
        );
        let inner = Self::pane(cx, buf, area, &title, false);
        if inner.height == 0 || inner.width == 0 {
            return;
        }
        // Horizontal bars only: the whole machine, a blank row, then the cores in two columns.
        let total = self.glide.cpu.map_or(s.cpu / 100.0, |bar| bar.value(self.frame_time));
        Self::draw_load(cx, buf, Rect { height: 1, ..inner }, "all", total);
        let top = inner.y + 2;
        let core_rows = (s.cores.len() as u16).div_ceil(2).min(inner.bottom().saturating_sub(top));
        if core_rows == 0 {
            return;
        }
        // A gap of two cells between the columns keeps a value off the next label.
        let half = inner.width.saturating_sub(GAP) / 2;
        for (i, load) in s.cores.iter().enumerate() {
            let row = i as u16 % core_rows;
            let column = i as u16 / core_rows;
            if column > 1 {
                break;
            }
            let cell = Rect {
                x: inner.x + column * (half + GAP),
                y: top + row,
                width: if column == 0 { half } else { inner.width - half - GAP },
                height: 1,
            };
            let ratio = shown(&self.glide.cores, i, self.frame_time, load / 100.0);
            Self::draw_load(cx, buf, cell, &i.to_string(), ratio);
        }
    }

    /// One load on one row: its label, a bar and its percent.
    fn draw_load(cx: &ui::Context, buf: &mut Buffer, area: Rect, label: &str, ratio: f32) {
        let value = format!("{:.0}%", ratio * 100.0);
        graph::MeterRow {
            label,
            label_width: 3,
            ratio,
            value: &value,
            value_width: 4,
            reversed: false,
        }
        .render(cx, area, buf);
    }

    /// One labeled meter over two rows (the label with its numbers, then the bar); returns the row of
    /// the next meter, after a blank one.
    fn draw_meter(area: Rect, buf: &mut Buffer, cx: &ui::Context, y: u16, gauge: Gauge) -> u16 {
        let theme = cx.theme;
        let Gauge { label, used, total, shown, notes } = gauge;
        if y + 1 >= area.bottom() {
            return area.bottom();
        }
        let r = ratio(used, total);
        let mut spans = vec![
            Span::raw(label.to_string()),
            Span::styled(
                format!("  {} / {}  {:.0}%", format::bytes(used), format::bytes(total), r * 100.0),
                Style::new().fg(theme.colors.muted.into()),
            ),
        ];
        spans.extend(notes);
        let line = Line::from(spans);
        let row = |y| Rect { y, height: 1, ..area };
        Paragraph::new(line).render(row(y), buf);
        graph::meter(
            buf,
            row(y + 1),
            shown,
            |place| graph::heat(theme, place),
            theme.colors.border.into(),
        );
        // A blank row before the next meter.
        y + 3
    }

    /// The memory pane: memory, swap and every disk.
    fn draw_memory(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let s = &self.sampler.snapshot;
        let inner = Self::pane(cx, buf, area, "memory", false);
        if inner.height == 0 || inner.width == 0 {
            return;
        }
        let now = self.frame_time;
        let mem_ratio = ratio(s.memory_used, s.memory_total);
        let swap_ratio = ratio(s.swap_used, s.swap_total);
        let memory = self.glide.memory.map_or(mem_ratio, |bar| bar.value(now));
        let swap = self.glide.swap.map_or(swap_ratio, |bar| bar.value(now));
        let mut y = inner.y;
        let gauge = Gauge::new("mem", s.memory_used, s.memory_total, memory);
        y = Self::draw_meter(inner, buf, cx, y, gauge);
        if s.swap_total > 0 {
            let gauge = Gauge::new("swap", s.swap_used, s.swap_total, swap);
            y = Self::draw_meter(inner, buf, cx, y, gauge);
        }
        let c = &cx.theme.colors;
        for (i, disk) in s.disks.iter().enumerate() {
            let fallback = ratio(disk.used, disk.total);
            let mut gauge = Gauge::new(
                &disk.mount,
                disk.used,
                disk.total,
                shown(&self.glide.disks, i, now, fallback),
            );
            let muted = Style::new().fg(c.muted.into());
            gauge.notes.push(Span::styled(
                format!("  r {}  w {}", format::rate(disk.read), format::rate(disk.written)),
                muted,
            ));
            match disk.health {
                Some(Health::Good) => gauge.notes.push(Span::styled("  healthy", muted)),
                Some(Health::Failing) => gauge.notes.push(Span::styled(
                    "  failing",
                    Style::new().fg(c.error.into()).add_modifier(Modifier::BOLD),
                )),
                None => {}
            }
            y = Self::draw_meter(inner, buf, cx, y, gauge);
        }
    }

    /// The sensors pane: the battery (a bar, whether it charges, how long it lasts), then every
    /// temperature as a bar toward its critical point.
    fn draw_sensors(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let s = &self.sampler.snapshot;
        let inner = Self::pane(cx, buf, area, "sensors", false);
        if inner.height == 0 || inner.width == 0 {
            return;
        }
        let muted = Style::new().fg(cx.theme.colors.muted.into());
        if s.battery.is_none() && s.temps.is_empty() {
            buf.set_stringn(
                inner.x,
                inner.y,
                "no sensors to read",
                usize::from(inner.width),
                muted,
            );
            return;
        }
        let row = |y| Rect { y, height: 1, ..inner };
        let mut y = inner.y;
        if let Some(b) = s.battery {
            let value = format!("{:.0}%", b.percent);
            graph::MeterRow {
                label: "battery",
                label_width: LABEL_W,
                ratio: b.percent / 100.0,
                value: &value,
                value_width: 4,
                reversed: true,
            }
            .render(cx, row(y), buf);
            if y + 1 < inner.bottom() {
                // Under a minute the estimate is noise (a battery at its end of charge).
                let left = b
                    .time_left
                    .filter(|t| *t >= 60)
                    .map(|t| format!("  {} left", format::duration(t)));
                let text = format!("{}{}", b.charge.label(), left.unwrap_or_default());
                buf.set_stringn(inner.x, y + 1, text, usize::from(inner.width), muted);
            }
            // A blank row before the temperatures.
            y += 3;
        }
        for t in &s.temps {
            if y >= inner.bottom() {
                break;
            }
            let value = format!("{:.0}°", t.celsius);
            graph::MeterRow {
                label: &t.label,
                label_width: LABEL_W,
                ratio: t.celsius / t.critical.filter(|c| *c > 0.0).unwrap_or(100.0),
                value: &value,
                value_width: 4,
                reversed: false,
            }
            .render(cx, row(y), buf);
            y += 1;
        }
    }

    /// The network pane: download over upload, each a label and a history graph.
    fn draw_network(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let theme = cx.theme;
        let s = &self.sampler.snapshot;
        let inner = Self::pane(cx, buf, area, "network", false);
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
            graph::history(buf, graph_area, &history.scaled(4096.0), |_| color.into());
        }
    }

    /// The columns of the process list, the sorted one marked.
    fn process_columns(options: &Options) -> Columns {
        Columns::new(
            vec![Column::right("PID", 7)],
            "NAME",
            vec![Column::right("CPU%", 5), Column::right("MEM", 7), Column::left("STATE", 8)],
            20,
        )
        .sorted_by(options.sort.column(), options.is_descending())
    }

    /// The process pane: a header, then the rows with the cursor.
    fn draw_procs(&mut self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let theme = cx.theme;
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
        let pane = Pane::new().title(&title).focused(true);
        let inner = pane.draw(cx, area, buf);
        if inner.height < 2 {
            return;
        }
        let muted = Style::new().fg(theme.colors.muted.into());
        let columns = Self::process_columns(&self.options);
        columns.draw_header(cx, buf, inner);

        let list = Rect { y: inner.y + 1, height: inner.height - 1, ..inner };
        let procs = &self.sampler.snapshot.procs;
        let depths: Vec<usize> = rows.iter().map(|r| r.depth).collect();
        let scope = tree::active_scope(&depths, self.list.cursor());
        let row = |s: Slot| {
            let row = &rows[s.index];
            let p = &procs[row.index];
            let cpu_color = graph::heat(theme, p.cpu / 100.0);
            let guides =
                tree::guides_marked(cx, row.depth, scope.and_then(|sc| sc.col_at(s.index)));
            let mut left = columns.lead_cells(vec![Span::styled(p.pid.to_string(), muted)]);
            left.extend(guides);
            left.push(Span::raw(p.name.clone()));
            // The row keeps its identity (the pid) and never flashes when the numbers change.
            ListRow {
                id: u64::from(p.pid) + 1,
                content: 1,
                left,
                right: columns.trail_cells(vec![
                    Span::styled(format!("{:.1}", p.cpu), Style::new().fg(cpu_color)),
                    Span::raw(format::bytes(p.memory)),
                    Span::raw(p.status.clone()),
                ]),
                min_left: columns.min_left(),
                ..ListRow::default()
            }
        };
        self.list.view(rows.len(), row).pane(area).focused(true).render(cx, list, buf);
    }

    /// The footer line: a prompt, a question, a message, or the hints.
    fn draw_footer(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
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
                None => Footer::Hint(format!(
                    "s sort  r reverse  t tree  / filter  x terminate  X kill  1-5 panels  ? help  q quit  every {}ms",
                    self.interval.as_millis()
                )),
            },
        };
        footer.render(cx, area, buf);
    }

    /// Feeds a key to the filter prompt or the question.
    fn typing(&mut self, ev: &Event) -> Request {
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Filter(mut input) => match ui::prompt::typed(&mut input, ev) {
                Typed::Submit(text) => {
                    self.options.filter = text;
                    self.list.reset();
                    self.follow_selected();
                }
                Typed::Cancel => {}
                Typed::Editing => {
                    self.options.filter = input.value().to_string();
                    self.list.set_cursor(0);
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
    fn render(&mut self, cx: &ui::Context, area: Rect, buf: &mut Buffer) {
        if area.height < 6 || area.width < 20 {
            return;
        }
        self.frame_time = Instant::now();
        let footer = Rect { y: area.bottom() - 1, height: 1, ..area };
        let body = Rect { height: area.height - 1, ..area };
        let placed = self.panels.place(body);
        if let Some(rect) = placed.cpu {
            self.draw_cpu(rect, buf, cx);
        }
        if let Some(rect) = placed.memory {
            self.draw_memory(rect, buf, cx);
        }
        if let Some(rect) = placed.sensors {
            self.draw_sensors(rect, buf, cx);
        }
        if let Some(rect) = placed.network {
            self.draw_network(rect, buf, cx);
        }
        if let Some(rect) = placed.processes {
            self.draw_procs(rect, buf, cx);
        }
        if self.glide.is_moving(self.frame_time) {
            cx.motion.request_frame();
        }
        self.draw_footer(footer, buf, cx);
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
            "panel.cpu" => self.toggle_panel(Panel::Cpu),
            "panel.memory" => self.toggle_panel(Panel::Memory),
            "panel.network" => self.toggle_panel(Panel::Network),
            "panel.processes" => self.toggle_panel(Panel::Processes),
            "panel.sensors" => self.toggle_panel(Panel::Sensors),
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

    /// Reads the machine once the interval has passed. A tick also comes for every frame of a
    /// glide, so most ticks only draw again.
    fn on_tick(&mut self) -> bool {
        if self.last_refresh.elapsed() >= self.interval {
            self.last_refresh = Instant::now();
            self.sampler.refresh();
            self.glide.retarget(&self.sampler.snapshot, self.last_refresh);
            self.follow_selected();
        }
        true
    }

    fn navigable(&mut self) -> Option<&mut dyn Navigable> {
        Some(self)
    }
}

/// The process rows are what the shared movement walks.
/// Every move also notes the process the cursor lands on, so it stays on it as the list changes.
impl Navigable for Monitor {
    fn move_by(&mut self, delta: isize) {
        self.list.move_by(delta);
        self.remember(&self.rows());
    }

    fn page_row_count(&self) -> usize {
        self.list.page_row_count()
    }

    fn move_to_top(&mut self) {
        self.list.move_to_top();
        self.remember(&self.rows());
    }

    fn move_to_bottom(&mut self) {
        self.list.move_to_bottom();
        self.remember(&self.rows());
    }
}
