//! Screen splitting. Tools ask for rects here instead of computing their own,
//! so the pane gap and ratios come from the theme.

use crate::theme::Theme;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Split `area` into columns with the theme's gap between them.
pub fn columns(theme: &Theme, area: Rect, constraints: &[Constraint]) -> Vec<Rect> {
    split(theme, area, Direction::Horizontal, constraints)
}

/// Split `area` into rows with the theme's gap between them.
pub fn rows(theme: &Theme, area: Rect, constraints: &[Constraint]) -> Vec<Rect> {
    split(theme, area, Direction::Vertical, constraints)
}

fn split(theme: &Theme, area: Rect, dir: Direction, constraints: &[Constraint]) -> Vec<Rect> {
    Layout::new(dir, constraints).spacing(theme.layout.gap).split(area).to_vec()
}

/// A popup centered in `area`, `popup_percent` of its width and height.
pub fn popup(theme: &Theme, area: Rect) -> Rect {
    let p = u32::from(theme.layout.popup_percent.clamp(10, 100));
    let w = (u32::from(area.width) * p / 100) as u16;
    let h = (u32::from(area.height) * p / 100) as u16;
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

/// `[sidebar | main | preview]` using the theme's sidebar width and preview share.
pub fn sidebar_main_preview(theme: &Theme, area: Rect) -> [Rect; 3] {
    let l = &theme.layout;
    let p = l.preview_percent.min(100);
    let rest = area.width.saturating_sub(l.sidebar_width + 2 * l.gap);
    let preview = rest * p / 100;
    let v = columns(
        theme,
        area,
        &[Constraint::Length(l.sidebar_width), Constraint::Min(0), Constraint::Length(preview)],
    );
    [v[0], v[1], v[2]]
}

/// `[main | preview]` using the theme's preview share.
pub fn main_preview(theme: &Theme, area: Rect) -> [Rect; 2] {
    let p = theme.layout.preview_percent.min(100);
    let v = columns(theme, area, &[Constraint::Percentage(100 - p), Constraint::Percentage(p)]);
    [v[0], v[1]]
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod popup_tests;

/// Where the docks, the editor and the ribbon go (see `workbench`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Workbench {
    pub left: Rect,
    pub right: Rect,
    pub bottom: Rect,
    /// What is left for the editor (its box, tabs and text).
    pub editor: Rect,
    /// The very bottom row, full width: the status lines. Empty (height 0) only
    /// when there is no room for it.
    pub ribbon: Rect,
}

/// Split `main` into a left dock, a right dock, a bottom dock, the editor and the
/// ribbon, like the side bars, panel and status bar of VS Code or Zed. The sizes
/// are widths and a height in cells; 0 means the dock is closed. Docks never take
/// more than a third of the width each or half of the height, and the editor
/// keeps a minimum size.
///
/// The ribbon is always the last row of `main`, across the whole width, below
/// every dock. The side docks run from the top down to just above it, and the
/// bottom dock sits under the editor between the side docks.
pub fn workbench(main: Rect, left: u16, right: u16, bottom: u16) -> Workbench {
    const MIN_EDITOR_WIDTH: u16 = 20;
    const MIN_EDITOR_HEIGHT: u16 = 4;
    let ribbon_h = u16::from(main.height > MIN_EDITOR_HEIGHT);
    let body = Rect { height: main.height - ribbon_h, ..main };
    // A side dock takes at most two fifths of the width (a focused files pane widens up to that for a long name).
    let third = body.width * 2 / 5;
    let mut left = left.min(third);
    let mut right = right.min(third);
    while body.width.saturating_sub(left + right) < MIN_EDITOR_WIDTH.min(body.width)
        && (left > 0 || right > 0)
    {
        if left >= right { left -= 1 } else { right -= 1 }
    }
    let bottom = bottom.min(body.height / 2).min(body.height.saturating_sub(MIN_EDITOR_HEIGHT));
    let mid_x = body.x + left;
    let mid_w = body.width - left - right;
    Workbench {
        left: Rect {
            x: body.x,
            y: body.y,
            width: left,
            height: if left == 0 { 0 } else { body.height },
        },
        right: Rect {
            x: body.right() - right,
            y: body.y,
            width: right,
            height: if right == 0 { 0 } else { body.height },
        },
        bottom: Rect {
            x: mid_x,
            y: body.bottom() - bottom,
            width: if bottom == 0 { 0 } else { mid_w },
            height: bottom,
        },
        editor: Rect { x: mid_x, y: body.y, width: mid_w, height: body.height - bottom },
        ribbon: Rect { x: main.x, y: body.bottom(), width: main.width, height: ribbon_h },
    }
}

#[cfg(test)]
mod workbench_tests;
