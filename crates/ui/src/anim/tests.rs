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
