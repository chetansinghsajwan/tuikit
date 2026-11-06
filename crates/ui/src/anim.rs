//! Animation: the one place that owns timing, easing and drawing of motion, so every tool moves alike.
//!
//! Tools do not animate. They draw as usual; `finish` (once per frame, after drawing) fades the cursor
//! row in, and `spinner` stands in for a `…` while something runs. Motion asks for the next frame itself:
//! `poll_timeout` is how long the event loop may wait, short while something moves and the tool's own
//! idle time otherwise, so a still screen costs nothing.
//!
//! `animations = "on" | "reduced" | "off"` in the config (`on` default): `reduced` keeps only the
//! spinner, `off` shows no motion at all. Fades need truecolor and are skipped without it.

use crate::theme::{Theme, mix};
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// How much motion is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    On,
    Reduced,
    Off,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "on" | "true" | "full" => Some(Mode::On),
            "reduced" => Some(Mode::Reduced),
            "off" | "false" | "none" => Some(Mode::Off),
            _ => None,
        }
    }
}

/// The frame time while something moves (about 30 frames a second).
pub const FRAME: Duration = Duration::from_millis(33);
/// How long the cursor row takes to fade in.
const FADE: Duration = Duration::from_millis(140);
/// How long one spinner frame shows.
const STEP: Duration = Duration::from_millis(80);
const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn start() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

fn mode() -> Mode {
    crate::config::get().animations.as_deref().and_then(Mode::parse).unwrap_or_default()
}

#[derive(Default)]
struct State {
    /// A frame is wanted soon: something is still moving.
    wanted: bool,
    /// Rows (by line) that carried the highlight in the last frame, and when each began.
    seen: HashMap<u16, Instant>,
    first: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State { first: true, ..State::default() });
}

fn want() {
    STATE.with(|s| s.borrow_mut().wanted = true);
}

/// A spinner frame for something running; asks for the next frame. A plain `…` when animations are off.
pub fn spinner() -> &'static str {
    if mode() == Mode::Off {
        return "…";
    }
    want();
    FRAMES[(start().elapsed().as_millis() / STEP.as_millis()) as usize % FRAMES.len()]
}

/// How long the event loop may wait for input: a frame while something moves, `idle` otherwise.
/// Call it after drawing, when the frame has said what it needs.
pub fn poll_timeout(idle: Duration) -> Duration {
    if STATE.with(|s| s.borrow().wanted) { FRAME.min(idle) } else { idle }
}

/// Whether the last frame was still moving, so the loop draws again even without input.
/// Call it when the wait for input timed out; it clears the request, and drawing sets it anew.
pub fn take_redraw() -> bool {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().wanted))
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Fade the cursor row in. Call once per frame, right after the tool has drawn into `buf`: rows that
/// carry the highlight and did not in the last frame blend in from the plain background.
pub fn finish(buf: &mut Buffer, theme: &Theme) {
    if mode() != Mode::On || !matches!(theme.colors.bg.into(), Color::Rgb(..)) {
        return;
    }
    let bg: Color = theme.colors.bg.into();
    let lit = mix(theme.colors.accent.into(), bg, 28);
    let now = Instant::now();
    let area = buf.area;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut seen = HashMap::new();
        let mut moving = false;
        for y in area.top()..area.bottom() {
            if !(area.left()..area.right()).any(|x| buf[(x, y)].bg == lit) {
                continue;
            }
            let began = if s.first { now - FADE } else { s.seen.get(&y).copied().unwrap_or(now) };
            seen.insert(y, began);
            let t = now.duration_since(began).as_secs_f32() / FADE.as_secs_f32();
            if t < 1.0 {
                moving = true;
                let shade = mix(lit, bg, (ease_out(t) * 100.0) as u8);
                for x in area.left()..area.right() {
                    if buf[(x, y)].bg == lit {
                        buf[(x, y)].set_bg(shade);
                    }
                }
            }
        }
        s.seen = seen;
        s.first = false;
        s.wanted |= moving;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    fn lit(theme: &Theme) -> Color {
        mix(theme.colors.accent.into(), theme.colors.bg.into(), 28)
    }

    fn paint(buf: &mut Buffer, theme: &Theme, y: u16) {
        for x in 0..buf.area.width {
            buf[(x, y)].set_bg(lit(theme));
        }
    }

    #[test]
    fn a_row_that_gains_the_highlight_fades_in_and_asks_for_frames() {
        let theme = Theme::default();
        let area = Rect::new(0, 0, 6, 3);
        let mut buf = Buffer::empty(area);
        paint(&mut buf, &theme, 0);
        finish(&mut buf, &theme); // the first frame shows the highlight as it is
        assert_eq!(buf[(0, 0)].bg, lit(&theme));

        let mut buf = Buffer::empty(area);
        paint(&mut buf, &theme, 2);
        finish(&mut buf, &theme);
        assert_ne!(buf[(0, 2)].bg, lit(&theme), "the new cursor row starts faded");
        assert_eq!(poll_timeout(Duration::from_secs(1)), FRAME);
        assert!(take_redraw());
        assert_eq!(poll_timeout(Duration::from_secs(1)), Duration::from_secs(1));
    }

    #[test]
    fn modes_parse() {
        assert_eq!(Mode::parse("Reduced"), Some(Mode::Reduced));
        assert_eq!(Mode::parse("off"), Some(Mode::Off));
        assert_eq!(Mode::parse("nope"), None);
    }

    #[test]
    fn the_spinner_cycles_through_braille_frames() {
        assert!(FRAMES.contains(&spinner()));
    }
}
