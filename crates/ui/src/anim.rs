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
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
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

/// One drawn row of a list, as the row component noted it.
#[derive(Clone)]
struct RowRec {
    y: u16,
    id: u64,
    content: u64,
    bar: Rect,
}

/// A glide of a list to its new scroll position: the frame as it was, and how far the content moved.
struct Scroll {
    from: Buffer,
    d: i32,
    began: Instant,
    rect: Rect,
}

#[derive(Default)]
struct PaneState {
    /// The rows of the last frame.
    rows: Vec<RowRec>,
    /// Rows that appeared (a fold opened): when each begins to show.
    born: HashMap<u64, Instant>,
    /// Rows whose content changed: when each flash began.
    flash: HashMap<u64, Instant>,
    scroll: Option<Scroll>,
}

#[derive(Default)]
struct State {
    /// A frame is wanted soon: something is still moving.
    wanted: bool,
    /// Rows (by line) that carried the highlight in the last frame, and when each began.
    seen: HashMap<u16, Instant>,
    first: bool,
    /// Rows noted while drawing this frame, by list.
    rows: HashMap<(u16, u16), Vec<RowRec>>,
    panes: HashMap<(u16, u16), PaneState>,
    /// What lay under each overlay (popup, menu) before it was drawn this frame.
    overlays: Vec<(Rect, Vec<Cell>)>,
    overlay_began: HashMap<Rect, Instant>,
    /// Tab strips noted this frame: (strip, active tab, the content below it).
    tabs: Vec<((u16, u16), usize, Rect)>,
    tab_active: HashMap<(u16, u16), usize>,
    tab_fades: HashMap<(u16, u16), Instant>,
    frame: Option<Buffer>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State { first: true, ..State::default() });
}

fn want() {
    STATE.with(|s| s.borrow_mut().wanted = true);
}

/// A stable number for `v`, for a row's identity and content.
pub fn hash_of(v: &impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

/// A row component notes each row it drew: `pane` identifies the list, `id` the row (its path), `content` what
/// it shows apart from the cursor. `finish` then glides a scrolled list, reveals rows a fold opened and flashes
/// rows whose content changed.
pub fn note_row(pane: (u16, u16), y: u16, id: u64, content: u64, bar: Rect) {
    STATE.with(|s| s.borrow_mut().rows.entry(pane).or_default().push(RowRec { y, id, content, bar }));
}

/// An overlay (popup, menu) is about to be drawn into `rect`: remember what lies under it, so it fades in over it.
pub fn overlay(buf: &Buffer, rect: Rect) {
    let rect = rect.intersection(buf.area);
    let under = (rect.top()..rect.bottom()).flat_map(|y| (rect.left()..rect.right()).map(move |x| (x, y))).map(|p| buf[p].clone()).collect();
    STATE.with(|s| s.borrow_mut().overlays.push((rect, under)));
}

/// A tab strip at `strip` shows tab `active` over `content`; when the tab changes, the content fades in.
pub fn tab(strip: (u16, u16), active: usize, content: Rect) {
    STATE.with(|s| s.borrow_mut().tabs.push((strip, active, content)));
}

const REVEAL: Duration = Duration::from_millis(140);
const STAGGER: Duration = Duration::from_millis(22);
const FLASH: Duration = Duration::from_millis(700);
const GLIDE: Duration = Duration::from_millis(160);
const OVERLAY: Duration = Duration::from_millis(100);
const TAB: Duration = Duration::from_millis(160);

fn progress(began: Instant, now: Instant, over: Duration) -> f32 {
    if now < began { 0.0 } else { (now.duration_since(began).as_secs_f32() / over.as_secs_f32()).min(1.0) }
}

fn rgb(c: Color, fallback: Color) -> Color {
    if matches!(c, Color::Rgb(..)) { c } else { fallback }
}

/// The union of the bars of `rows`.
fn bounds(rows: &[RowRec]) -> Rect {
    rows.iter().map(|r| r.bar).reduce(|a, b| a.union(b)).unwrap_or_default()
}

/// How far the rows moved between two frames, when the whole list scrolled (not when a fold opened or closed:
/// there the rows above the change stay put).
fn scrolled(prev: &[RowRec], cur: &[RowRec]) -> Option<i32> {
    let before: HashMap<u64, u16> = prev.iter().map(|r| (r.id, r.y)).collect();
    let mut shifts: HashMap<i32, usize> = HashMap::new();
    let (mut still, mut matched) = (0, 0);
    for r in cur {
        if let Some(&y) = before.get(&r.id) {
            matched += 1;
            let d = i32::from(y) - i32::from(r.y);
            if d == 0 {
                still += 1;
            } else {
                *shifts.entry(d).or_default() += 1;
            }
        }
    }
    let (d, n) = shifts.into_iter().max_by_key(|&(_, n)| n)?;
    // Pinned parent rows stay put; more than a few that do means a fold, not a scroll.
    (d.abs() >= 2 && n >= 3 && n * 10 >= matched * 7 && still <= 3).then_some(d)
}

/// The motion of the frame just drawn into `buf`. Call once per frame, right after the tool has drawn:
/// lists glide to a new scroll position, rows a fold opened reveal one by one, rows that changed flash, popups
/// and new tabs fade in, and the cursor row fades in. All of it only recolors or moves what the tool drew.
pub fn finish(buf: &mut Buffer, theme: &Theme) {
    let on = mode() == Mode::On && matches!(theme.colors.bg.into(), Color::Rgb(..));
    let bg: Color = theme.colors.bg.into();
    let now = Instant::now();
    let area = buf.area;
    let truth = buf.clone();
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        let st = &mut *st;
        let rows = std::mem::take(&mut st.rows);
        let overlays = std::mem::take(&mut st.overlays);
        let tabs = std::mem::take(&mut st.tabs);
        let frame = st.frame.replace(truth.clone());
        if !on {
            st.panes.clear();
            return;
        }
        let steady = frame.as_ref().is_some_and(|f| f.area == area);
        let mut moving = false;
        let mut gliding = false;

        // Lists.
        st.panes.retain(|k, _| rows.contains_key(k));
        for (key, cur) in rows {
            let pane = st.panes.entry(key).or_default();
            let prev = std::mem::replace(&mut pane.rows, cur.clone());
            let mut started = false;
            if steady && !prev.is_empty() {
                if let Some(d) = scrolled(&prev, &cur) {
                    pane.scroll = frame.clone().map(|from| Scroll { from, d, began: now, rect: bounds(&cur) });
                    started = true;
                } else if pane.scroll.is_none() {
                    let old: HashMap<u64, usize> = prev.iter().enumerate().map(|(i, r)| (r.id, i)).collect();
                    let mut run = 0;
                    for (i, r) in cur.iter().enumerate() {
                        match old.get(&r.id) {
                            None => {
                                // New in the middle of rows that were there: a fold opened. Rows at the edges are scrolling in.
                                let inside = cur[..i].iter().any(|p| old.contains_key(&p.id)) && cur[i..].iter().any(|n| old.contains_key(&n.id));
                                if inside {
                                    pane.born.insert(r.id, now + STAGGER * run);
                                    run += 1;
                                }
                            }
                            Some(&j) => {
                                run = 0;
                                if prev[j].content != r.content {
                                    pane.flash.insert(r.id, now);
                                }
                            }
                        }
                    }
                }
            }
            let _ = started;

            // The glide: show the frames between the old and the new position.
            if let Some(sc) = &pane.scroll {
                let t = progress(sc.began, now, GLIDE);
                if t >= 1.0 || sc.from.area != area {
                    pane.scroll = None;
                } else {
                    gliding = true;
                    moving = true;
                    let shift = (sc.d.abs() as f32 * (1.0 - ease_out(t))).round() as i32 * sc.d.signum();
                    let r = sc.rect;
                    for y in r.top()..r.bottom() {
                        let v = i32::from(y) - shift;
                        for x in r.left()..r.right() {
                            let src = if v >= i32::from(r.top()) && v < i32::from(r.bottom()) {
                                Some(&truth[(x, v as u16)])
                            } else {
                                let o = v + sc.d;
                                (o >= i32::from(r.top()) && o < i32::from(r.bottom())).then(|| &sc.from[(x, o as u16)])
                            };
                            buf[(x, y)] = src.cloned().unwrap_or_default();
                        }
                    }
                }
            }

            // Reveal and flash on the rows as drawn (not while the list glides: the rows are elsewhere).
            if pane.scroll.is_none() {
                pane.born.retain(|_, b| progress(*b, now, REVEAL) < 1.0);
                pane.flash.retain(|_, b| progress(*b, now, FLASH) < 1.0);
                for r in &cur {
                    let reveal = pane.born.get(&r.id).map(|b| progress(*b, now, REVEAL));
                    let flash = pane.flash.get(&r.id).map(|b| ease_out(progress(*b, now, FLASH)));
                    if reveal.is_none() && flash.is_none() {
                        continue;
                    }
                    moving = true;
                    for x in r.bar.left()..r.bar.right() {
                        let cell = &mut buf[(x, r.y)];
                        let cbg = rgb(cell.bg, bg);
                        if let Some(p) = reveal {
                            if let Color::Rgb(..) = cell.fg {
                                cell.set_fg(mix(cell.fg, cbg, (p * 100.0) as u8));
                            }
                        }
                        if let Some(e) = flash {
                            cell.set_bg(mix(theme.colors.accent.into(), cbg, (24.0 * (1.0 - e)) as u8));
                        }
                    }
                }
            }
        }

        // Overlays fade in over what they cover.
        let mut began = HashMap::new();
        for (rect, under) in overlays {
            let b = st.overlay_began.get(&rect).copied().unwrap_or(now);
            began.insert(rect, b);
            let p = progress(b, now, OVERLAY);
            if p >= 1.0 {
                continue;
            }
            moving = true;
            let mut it = under.into_iter();
            for y in rect.top()..rect.bottom() {
                for x in rect.left()..rect.right() {
                    let Some(u) = it.next() else { break };
                    let cell = &mut buf[(x, y)];
                    let shade = mix(rgb(cell.bg, bg), rgb(u.bg, bg), (ease_out(p) * 100.0) as u8);
                    let ink = mix(rgb(cell.fg, bg), shade, (ease_out(p) * 100.0) as u8);
                    cell.set_bg(shade);
                    if matches!(cell.fg, Color::Rgb(..)) {
                        cell.set_fg(ink);
                    }
                }
            }
        }
        st.overlay_began = began;

        // A new tab: its content fades in.
        for (strip, active, content) in tabs {
            if st.tab_active.insert(strip, active).is_some_and(|was| was != active) && steady {
                st.tab_fades.insert(strip, now);
            }
            if let Some(b) = st.tab_fades.get(&strip).copied() {
                let p = progress(b, now, TAB);
                if p >= 1.0 {
                    st.tab_fades.remove(&strip);
                    continue;
                }
                moving = true;
                let r = content.intersection(area);
                for y in r.top()..r.bottom() {
                    for x in r.left()..r.right() {
                        let cell = &mut buf[(x, y)];
                        if let Color::Rgb(..) = cell.fg {
                            cell.set_fg(mix(cell.fg, rgb(cell.bg, bg), (ease_out(p) * 100.0) as u8));
                        }
                    }
                }
            }
        }

        // The cursor row, judged on the frame as the tool drew it.
        let lit = mix(theme.colors.accent.into(), bg, 28);
        let mut seen = HashMap::new();
        for y in area.top()..area.bottom() {
            if !(area.left()..area.right()).any(|x| truth[(x, y)].bg == lit) {
                continue;
            }
            let began = if st.first { now - FADE } else { st.seen.get(&y).copied().unwrap_or(now) };
            seen.insert(y, began);
            let t = now.duration_since(began).as_secs_f32() / FADE.as_secs_f32();
            if t < 1.0 {
                moving = true;
                if !gliding {
                    let shade = mix(lit, bg, (ease_out(t) * 100.0) as u8);
                    for x in area.left()..area.right() {
                        if buf[(x, y)].bg == lit {
                            buf[(x, y)].set_bg(shade);
                        }
                    }
                }
            }
        }
        st.seen = seen;
        st.first = false;
        st.wanted |= moving;
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

#[cfg(test)]
mod motion_tests {
    use super::*;

    fn theme() -> Theme {
        Theme::default()
    }

    /// A frame of `n` rows named by `names`, noted as a list at (0,0).
    fn frame(names: &[&str], rows: usize) -> Buffer {
        let area = Rect::new(0, 0, 8, rows as u16);
        let mut buf = Buffer::empty(area);
        for (y, name) in names.iter().enumerate() {
            let fg = Color::Rgb(200, 200, 200);
            buf.set_string(0, y as u16, name, ratatui::style::Style::new().fg(fg).bg(theme().colors.bg.into()));
            note_row((0, 0), y as u16, hash_of(name), 0, Rect::new(0, y as u16, 8, 1));
        }
        buf
    }

    fn names(buf: &Buffer) -> Vec<String> {
        (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>().trim().to_string()).collect()
    }

    #[test]
    fn a_big_scroll_glides_from_the_old_rows() {
        let t = theme();
        let all: Vec<String> = (0..20).map(|i| format!("r{i:02}")).collect();
        let win = |o: usize| all[o..o + 6].iter().map(String::as_str).collect::<Vec<_>>();
        let mut b = frame(&win(0), 6);
        finish(&mut b, &t);
        let mut b = frame(&win(3), 6);
        finish(&mut b, &t);
        assert_eq!(names(&b), ["r00", "r01", "r02", "r03", "r04", "r05"].map(String::from), "the first glide frame is still the old rows");
        assert_eq!(poll_timeout(Duration::from_secs(1)), FRAME);
    }

    #[test]
    fn a_fold_opening_reveals_the_new_rows_and_a_single_step_does_not_glide() {
        let t = theme();
        let mut b = frame(&["a", "b", "c", "d"], 6);
        finish(&mut b, &t);
        let mut b = frame(&["a", "b", "b1", "b2", "c", "d"], 6);
        finish(&mut b, &t);
        // The new row starts invisible: its ink equals its background.
        assert_eq!(b[(0, 2)].fg, Color::from(t.colors.bg), "revealed row starts faded");
        assert_eq!(b[(0, 4)].fg, Color::Rgb(200, 200, 200), "old rows are untouched");
    }

    #[test]
    fn a_changed_row_flashes() {
        let t = theme();
        let mut b = frame(&["a", "b"], 2);
        finish(&mut b, &t);
        let area = Rect::new(0, 0, 8, 2);
        let mut b = Buffer::empty(area);
        for (y, c) in [(0u16, 0u64), (1, 1)] {
            let name = ["a", "b"][y as usize];
            b.set_string(0, y, name, ratatui::style::Style::new().fg(Color::Rgb(200, 200, 200)).bg(t.colors.bg.into()));
            note_row((0, 0), y, hash_of(&name), c, Rect::new(0, y, 8, 1));
        }
        finish(&mut b, &t);
        assert_ne!(b[(0, 1)].bg, Color::from(t.colors.bg), "the changed row glows");
        assert_eq!(b[(0, 0)].bg, Color::from(t.colors.bg));
    }

    #[test]
    fn an_overlay_fades_in_over_what_was_under_it_and_a_tab_change_fades_the_content() {
        let t = theme();
        let area = Rect::new(0, 0, 8, 4);
        let mut b = Buffer::empty(area);
        for p in area.positions() {
            b[p].set_bg(t.colors.bg.into());
        }
        overlay(&b, Rect::new(1, 1, 4, 2));
        for p in Rect::new(1, 1, 4, 2).positions() {
            b[p].set_bg(Color::Rgb(255, 0, 0)).set_fg(Color::Rgb(255, 255, 255));
        }
        finish(&mut b, &t);
        assert_ne!(b[(1, 1)].bg, Color::Rgb(255, 0, 0), "the overlay starts faded");

        let mut b = Buffer::empty(area);
        for p in area.positions() {
            b[p].set_bg(t.colors.bg.into()).set_fg(Color::Rgb(255, 255, 255));
        }
        tab((0, 0), 0, Rect::new(0, 1, 8, 3));
        finish(&mut b, &t);
        assert_eq!(b[(0, 2)].fg, Color::Rgb(255, 255, 255), "no change of tab, no fade");
        let mut b = b.clone();
        tab((0, 0), 1, Rect::new(0, 1, 8, 3));
        finish(&mut b, &t);
        assert_ne!(b[(0, 2)].fg, Color::Rgb(255, 255, 255));
    }
}
