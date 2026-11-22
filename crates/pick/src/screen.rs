//! The picker as a screen of the shared shell, for the tools that run it on the terminal
//! (`pick` itself and the picker-style tools such as `ev`).

use crate::{Action, Picker};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use ui::app::{Host, Request, Screen};

/// The picker as a screen of the shared shell, which owns the terminal. Every event is the
/// picker's own (it has its own keys), so the shell's keymap stays out of it.
pub struct PickScreen {
    picker: Picker,
    /// Set by the background sources when they have news; the tick takes it.
    wake: Arc<AtomicBool>,
    /// How the picker ended.
    result: Action,
}

impl PickScreen {
    /// A screen for `picker`; `wake` is the flag its notify callback sets.
    pub fn new(picker: Picker, wake: Arc<AtomicBool>) -> Self {
        PickScreen { picker, wake, result: Action::Cancel }
    }

    /// How the picker ended: the selection, or a cancel.
    pub fn into_result(self) -> Action {
        self.result
    }
}

impl Screen for PickScreen {
    /// Draws the picker.
    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &ui::Theme) {
        self.picker.render(area, buf, theme);
    }

    /// The picker takes every event.
    fn captures_input(&self, _ev: &Event) -> bool {
        true
    }

    /// Feeds a key to the picker and quits once it accepts or cancels.
    fn on_event(&mut self, ev: &Event, _host: &mut dyn Host) -> Request {
        if let Event::Key(_) = ev {
            match self.picker.handle_event(ev) {
                Action::None => {}
                done => {
                    self.result = done;
                    return Request::Quit;
                }
            }
        }
        Request::None
    }

    /// The picker's preview, for the mouse wheel.
    fn preview_scroll(&mut self) -> Option<&mut ui::scroll::PreviewScroll> {
        Some(self.picker.preview_scroll())
    }

    /// The picker has no shell actions.
    fn run_action(&mut self, _id: &'static str, _host: &mut dyn Host) -> Request {
        Request::None
    }

    /// Lets the picker take what the background sources found.
    fn on_tick(&mut self) -> bool {
        self.wake.store(false, Ordering::Relaxed);
        self.picker.tick()
    }
}
