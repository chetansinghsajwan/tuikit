//! The scroll state of a preview pane, shared by every tool that has one: the offset, the keys
//! (`preview.*` actions) and the mouse wheel. A tool draws its lines from [`PreviewScroll::window`]
//! and tells the state its pane and the line count with [`PreviewScroll::view`]; it never keeps a
//! scroll counter of its own.

use crate::keys::Action;
use ratatui::crossterm::event::{Event, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

/// The shared preview actions, registered when the screen offers a [`PreviewScroll`]. `preview.up` and
/// `preview.down` scroll half a page (`ctrl-u`, `ctrl-d`); `top` and `bottom` have no default key and
/// are there for the palette and for the user's own bindings.
pub const ACTIONS: &[Action] = &[
    Action {
        id: "preview.up",
        name: "scroll preview up",
        desc: "scroll the preview up half a page",
        basic: true,
    },
    Action {
        id: "preview.down",
        name: "scroll preview down",
        desc: "scroll the preview down half a page",
        basic: true,
    },
    Action {
        id: "preview.top",
        name: "preview top",
        desc: "scroll the preview to its first line",
        basic: false,
    },
    Action {
        id: "preview.bottom",
        name: "preview bottom",
        desc: "scroll the preview to its last line",
        basic: false,
    },
];

/// Rows one wheel notch scrolls.
const WHEEL_ROWS: usize = 3;

/// Where a preview is scrolled to, and the room it has.
#[derive(Debug, Clone, Default)]
pub struct PreviewScroll {
    offset: usize,
    /// The rows the pane shows.
    height: usize,
    /// The lines the preview has.
    len: usize,
    /// Where the pane is on the screen, for the wheel.
    area: Rect,
}

impl PreviewScroll {
    /// The rows the pane shows (0 before the first draw).
    pub fn height(&self) -> usize {
        self.height
    }

    /// The first line shown.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// The lines to draw out of `lines`: from the offset, as many as the pane shows.
    pub fn window<'a, T>(&self, lines: &'a [T]) -> &'a [T] {
        let from = self.offset.min(lines.len());
        &lines[from..lines.len().min(from + self.height.max(1))]
    }

    /// Tells the state where the pane is (`area` is the rows the lines are drawn in) and how many lines
    /// there are. Call it on every draw: it keeps the offset inside the preview when the pane
    /// grows or the preview changes.
    pub fn view(&mut self, area: Rect, len: usize) {
        self.area = area;
        self.height = usize::from(area.height);
        self.len = len;
        self.offset = self.offset.min(self.max());
    }

    /// The last offset: the last line sits at the bottom of the pane.
    fn max(&self) -> usize {
        self.len.saturating_sub(self.height)
    }

    /// Scrolls to the first line. Call it when another thing is previewed.
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// Scrolls to the first line.
    pub fn top(&mut self) {
        self.offset = 0;
    }

    /// Scrolls to the end.
    pub fn bottom(&mut self) {
        self.offset = self.max();
    }

    /// Scrolls by `delta` rows (negative is up), stopping at both ends.
    pub fn by(&mut self, delta: isize) {
        self.offset = self.offset.saturating_add_signed(delta).min(self.max());
    }

    /// Scrolls down half a page.
    pub fn half_down(&mut self) {
        self.by(self.half());
    }

    /// Scrolls up half a page.
    pub fn half_up(&mut self) {
        self.by(-self.half());
    }

    fn half(&self) -> isize {
        (self.height / 2).max(1) as isize
    }

    /// Scrolls so that line `row` is the first one shown (as far as the end allows).
    pub fn to(&mut self, row: usize) {
        self.offset = row.min(self.max());
    }

    /// Runs the shared action `id`; false when `id` is not one of [`ACTIONS`].
    pub fn run(&mut self, id: &str) -> bool {
        match id {
            "preview.up" => self.half_up(),
            "preview.down" => self.half_down(),
            "preview.top" => self.top(),
            "preview.bottom" => self.bottom(),
            _ => return false,
        }
        true
    }

    /// Scrolls for a wheel notch over the pane; false when `ev` is not one (or is over something else).
    pub fn wheel(&mut self, ev: &Event) -> bool {
        let Event::Mouse(MouseEvent { kind, column, row, .. }) = ev else { return false };
        let inside = self.area.contains((*column, *row).into());
        match kind {
            MouseEventKind::ScrollDown if inside => self.by(WHEEL_ROWS as isize),
            MouseEventKind::ScrollUp if inside => self.by(-(WHEEL_ROWS as isize)),
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests;
