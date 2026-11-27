//! The monitor screen: the CPU, memory, network and process panes, and the state that is the
//! user's (cursor, sort, filter, tree, a signal waiting for a yes).

use crate::panels::{Panel, Panels};
use crate::procs::{self, Options, Order, Row, Sort};
use crate::sample::Sampler;
use glide::Glide;
use panes::process_columns;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};
use sysinfo::Signal;
use tui_input::Input;
use ui::Component;
use ui::app::{Host, Navigable, Request, Screen};
use ui::footer::{Footer, Notice};
use ui::prompt::Typed;
use ui::table::Table;

/// The values the bars show, each gliding to the latest reading.
mod glide;
/// The panes, one file each.
mod panes;

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
    /// The cursor, the scroll and the sort of the process rows.
    list: Table,
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
            options: Options::default(),
            mode: Mode::Normal,
            message: None,
            list: Table::new().with_sort(Sort::Cpu.column(), Sort::Cpu.is_descending()),
            selected: None,
        })
    }

    /// The rows of the process list now.
    fn rows(&self) -> Vec<Row> {
        procs::rows(&self.sampler.snapshot.procs, &self.options, self.order())
    }

    /// The order the process table is sorted in.
    fn order(&self) -> Order {
        let sort = self.list.sort();
        Order {
            sort: sort.and_then(|s| Sort::of_column(s.column)).unwrap_or(Sort::Cpu),
            is_descending: sort.is_none_or(|s| s.descending),
        }
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
                self.list.sort_next(&process_columns());
                self.follow_selected();
            }
            "sort.reverse" => {
                self.list.reverse_sort();
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

/// Tests for the monitor screen.
#[cfg(test)]
mod tests;
