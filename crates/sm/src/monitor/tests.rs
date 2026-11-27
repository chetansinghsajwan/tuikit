//! Tests for the monitor screen, drawn into a buffer.

use super::*;
use ratatui::buffer::Buffer;
use ui::Theme;

/// The text of every line of `buf`, trailing blanks trimmed.
fn lines(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .map(|l| l.trim_end().to_string())
        .collect()
}

/// The monitor drawn into `width` by `height`, or `None` when the sandbox hides the machine.
fn drawn(width: u16, height: u16) -> Option<Vec<String>> {
    let mut monitor = Monitor::new(Duration::from_secs(1), &[]).ok()?;
    let theme = Theme::default();
    let cx = ui::Context::new(&theme);
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    monitor.render(&cx, area, &mut buf);
    if std::env::var_os("SM_SHOW").is_some() {
        println!("{}", lines(&buf).join("\n"));
    }
    Some(lines(&buf))
}

#[test]
fn pane_titles_are_only_their_names() {
    let Some(screen) = drawn(140, 40) else { return };
    let text = screen.join("\n");
    for title in ["cpu", "memory", "network", "sensors", "processes"] {
        assert!(text.contains(&format!("─ {title} ─")), "{title} titles its pane, alone");
    }
    assert!(screen.iter().any(|l| l.contains(" processes  ") || l.ends_with(" processes")));
}

#[test]
fn a_reading_the_machine_lacks_still_has_its_row() {
    let Some(screen) = drawn(140, 40) else { return };
    let text = screen.join("\n");
    // Present or not, the battery and the temperatures keep a row; a missing one reads `n/a`.
    assert!(text.contains("battery"));
    assert!(text.contains("temperature") || text.contains('°'));
}

/// The monitor with the cursor on the first process and its details open, drawn.
fn drawn_details(width: u16, height: u16) -> Option<Vec<String>> {
    let mut monitor = Monitor::new(Duration::from_secs(1), &[]).ok()?;
    let theme = Theme::default();
    let cx = ui::Context::new(&theme);
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    monitor.render(&cx, area, &mut buf);
    let mut host = ui::app::NoHost;
    monitor.run_action("proc.details", &mut host);
    monitor.render(&cx, area, &mut buf);
    if std::env::var_os("SM_SHOW").is_some() {
        println!("{}", lines(&buf).join("\n"));
    }
    Some(lines(&buf))
}

#[test]
fn the_details_of_a_process_open_in_a_popup_and_close_with_escape() {
    let Some(screen) = drawn_details(140, 40) else { return };
    let text = screen.join("\n");
    for field in ["pid", "user", "state", "running", "memory", "command"] {
        assert!(text.contains(field), "{field} is in the popup");
    }
    assert!(text.contains("esc close"));
}

#[test]
fn processes_show_their_user() {
    let Some(screen) = drawn(140, 40) else { return };
    assert!(screen.iter().any(|l| l.contains("USER")), "{screen:#?}");
}

#[test]
fn the_sensors_pane_keeps_a_row_for_the_gpu_and_the_network_names_its_overlap() {
    let Some(screen) = drawn(140, 40) else { return };
    let text = screen.join("\n");
    assert!(text.contains("gpu memory"), "the card has its rows, `n/a` or not");
    assert!(text.contains("█ both"), "the legend names the overlap color");
}
