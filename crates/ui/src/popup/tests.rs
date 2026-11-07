use super::*;

#[test]
fn quoting_round_trips_through_sh() {
    for s in ["plain", "with space", "it's", "a\"b$c`d", "new\nline", "", "--flag=1"] {
        let out =
            Command::new("sh").arg("-c").arg(format!("printf %s {}", quote(s))).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), s, "{s:?}");
    }
}

#[test]
fn the_pane_command_is_a_centered_floating_blocking_pane() {
    let args: Vec<OsString> =
        vec!["--files".into(), "my dir".into(), "--title".into(), "it's".into()];
    let z = zellij_args(
        Path::new("/bin/pick"),
        &args,
        Path::new("/work"),
        Some(Path::new("/t/in")),
        Path::new("/t/out"),
        Path::new("/t/code"),
        80,
        None,
        &[("PATH".into(), "/a:/b".into())],
    );
    let has = |a: &str, b: &str| z.windows(2).any(|w| w[0] == a && w[1] == b);
    assert!(
        z.contains(&"--floating".to_string())
            && z.contains(&"--blocking".to_string())
            && z.contains(&"--close-on-exit".to_string())
    );
    assert!(has("--borderless", "true"));
    assert!(
        has("--width", "80%")
            && has("--height", "80%")
            && has("--x", "10%")
            && has("--y", "10%")
            && has("--cwd", "/work")
    );
    let script = z.last().unwrap();
    assert_eq!(
        script,
        "PATH='/a:/b' '/bin/pick' '--files' 'my dir' '--title' 'it'\\''s' --no-popup < '/t/in' > '/t/out'; echo $? > '/t/code'"
    );
    // No piped input: no redirect from `in`. Other sizes are centered too.
    let z = zellij_args(
        Path::new("/bin/pick"),
        &[],
        Path::new("/w"),
        None,
        Path::new("/o"),
        Path::new("/c"),
        60,
        None,
        &[],
    );
    assert!(
        !z.last().unwrap().contains(" < ") && z.windows(2).any(|w| w[0] == "--x" && w[1] == "20%")
    );
}

#[test]
fn popup_applies_only_inside_zellij_and_can_be_turned_off() {
    // Flag off always wins; the environment cases are covered by the end-to-end run.
    assert!(!wanted(true));
}

#[test]
fn the_vertical_offset_leaves_about_equal_room_above_and_below() {
    // Zellij: offset rows = rows * y / 100 - 1 (at least 0), height rows = rows * p / 100 rounded up.
    for rows in [24u16, 30, 37, 40, 50, 60, 100] {
        for p in [60u16, 80, 90] {
            let y = vertical_offset(p, rows);
            let height = (i32::from(rows) * i32::from(p) + 99) / 100;
            let top = (i32::from(rows) * i32::from(y) / 100 - 1).max(0);
            let bottom = i32::from(rows) - top - height;
            assert!(
                (top - bottom).abs() <= 2,
                "rows {rows} p {p}: y {y} top {top} bottom {bottom}"
            );
        }
    }
    // Full height needs no offset.
    assert_eq!(vertical_offset(100, 40), 0);
}
