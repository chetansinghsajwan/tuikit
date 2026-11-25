//! `Component` and `Context`: the one shape every drawn part of a tool takes, and the context it draws in.
//!
//! A component is built from its content (its props) and drawn once, by value, like a ratatui
//! `Widget`. What every component needs and no caller should pass by hand, the theme and the motion
//! memory, comes from the `Context` instead, so it is not threaded through each call and a test builds its
//! own `Context` with a fresh `Motion` instead of sharing the process-wide one.

use crate::anim::{self, Motion};
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// What a component draws with: the theme, and the motion memory it reports what it drew to.
#[derive(Clone, Copy)]
pub struct Context<'a> {
    pub theme: &'a Theme,
    pub motion: &'a Motion,
}

impl<'a> Context<'a> {
    /// A context on `theme` with the process-wide motion, the one the shell finishes each frame.
    pub fn new(theme: &'a Theme) -> Context<'a> {
        Context { theme, motion: anim::default_motion() }
    }

    /// A context on `theme` with its own `motion`, for tests and for drawing off screen.
    pub fn with_motion(theme: &'a Theme, motion: &'a Motion) -> Context<'a> {
        Context { theme, motion }
    }
}

/// A drawn part of a screen: built from its content, then drawn into `area` with `cx`.
pub trait Component {
    fn render(self, cx: &Context, area: Rect, buf: &mut Buffer);
}

#[cfg(test)]
mod tests;
