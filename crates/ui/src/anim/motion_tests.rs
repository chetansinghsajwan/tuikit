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
        buf.set_string(
            0,
            y as u16,
            name,
            ratatui::style::Style::new().fg(fg).bg(theme().colors.bg.into()),
        );
        note_row((0, 0), y as u16, hash_of(name), 0, Rect::new(0, y as u16, 8, 1));
    }
    buf
}

fn names(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim()
                .to_string()
        })
        .collect()
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
    assert_eq!(
        names(&b),
        ["r00", "r01", "r02", "r03", "r04", "r05"].map(String::from),
        "the first glide frame is still the old rows"
    );
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
        b.set_string(
            0,
            y,
            name,
            ratatui::style::Style::new().fg(Color::Rgb(200, 200, 200)).bg(t.colors.bg.into()),
        );
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
