//! Popup mode: inside zellij, a tool (`pick`, `git-panel`) re-runs itself in a floating pane (a real
//! overlay: the screen behind it is untouched) and relays the result. Input and
//! output go through temp files because a new pane does not share this
//! process's stdin and stdout; the exit code comes back through a file too.

use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Environment variables the pane should see as this process does (a new pane
/// otherwise inherits the zellij session's environment).
const FORWARD_ENV: [&str; 3] = ["PATH", "UI_THEME", "XDG_CONFIG_HOME"];

/// Popup mode applies inside zellij, unless turned off.
pub fn wanted(no_popup_flag: bool) -> bool {
    !no_popup_flag && std::env::var_os("ZELLIJ").is_some() && std::env::var_os("PICK_NO_POPUP").is_none()
}

/// Single-quote `s` for `sh`.
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The `--y` percent that centers a floating pane `p` percent tall in about `rows` rows.
/// Zellij turns the percentages into rows by rounding the height up and the offset down (minus
/// one row), which leaves the pane too high when it is just half the leftover; this picks the
/// smallest percent whose offset reaches the middle of the leftover room.
pub fn vertical_offset(p: u16, rows: u16) -> u16 {
    let (r, p) = (i32::from(rows), i32::from(p));
    let height = (r * p + 99) / 100;
    let want = ((r - height).max(0) + 1) / 2;
    for y in 0..=(100 - p) {
        if (r * y / 100 - 1).max(0) >= want {
            return y as u16;
        }
    }
    (100 - p) as u16
}

/// Arguments for `zellij`: a floating pane `percent` of the screen, centered,
/// that runs `exe args...` with input from `input` (if any) and output to
/// `out`, then records the exit code in `code`.
#[allow(clippy::too_many_arguments)]
pub fn zellij_args(exe: &Path, args: &[OsString], cwd: &Path, input: Option<&Path>, out: &Path, code: &Path, percent: u16, rows: Option<u16>, env: &[(String, String)]) -> Vec<String> {
    let p = percent.clamp(10, 100);
    let offset = rows.map_or((100 - p) / 2, |r| vertical_offset(p, r));
    let side = (100 - p) / 2;
    let q = |s: &OsStr| quote(&s.to_string_lossy());

    let mut script = String::new();
    for (k, v) in env {
        script.push_str(&format!("{k}={} ", quote(v)));
    }
    script.push_str(&q(exe.as_os_str()));
    for a in args {
        script.push(' ');
        script.push_str(&q(a));
    }
    script.push_str(" --no-popup");
    if let Some(i) = input {
        script.push_str(&format!(" < {}", q(i.as_os_str())));
    }
    script.push_str(&format!(" > {}; echo $? > {}", q(out.as_os_str()), q(code.as_os_str())));

    let mut v: Vec<String> = ["run", "--floating", "--close-on-exit", "--blocking", "--name", "pick"].map(String::from).to_vec();
    // pick draws its own (design-token) border, so the pane has none of its own.
    v.extend(["--borderless".into(), "true".into()]);
    v.extend(["--cwd".into(), cwd.to_string_lossy().into_owned()]);
    v.extend(["--width".into(), format!("{p}%"), "--height".into(), format!("{p}%")]);
    v.extend(["--x".into(), format!("{side}%"), "--y".into(), format!("{offset}%")]);
    v.extend(["--".into(), "sh".into(), "-c".into(), script]);
    v
}

/// Run `pick` with `args` in a floating pane and return its exit code. The
/// selection is written to this process's stdout. `Err` means the popup could
/// not be opened (the caller can fall back to this terminal).
pub fn run(args: Vec<OsString>, percent: u16) -> io::Result<i32> {
    let dir: PathBuf = std::env::temp_dir().join(format!("pick-popup-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let result = run_in(&dir, args, percent);
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn run_in(dir: &Path, args: Vec<OsString>, percent: u16) -> io::Result<i32> {
    let (input, out, code) = (dir.join("in"), dir.join("out"), dir.join("code"));

    // The pane has its own terminal, so piped input is handed over as a file.
    let has_input = !io::stdin().is_terminal();
    if has_input {
        let mut data = Vec::new();
        io::stdin().lock().read_to_end(&mut data)?;
        std::fs::write(&input, data)?;
    }

    let exe = std::env::current_exe()?;
    let cwd = std::env::current_dir()?;
    let env: Vec<(String, String)> = FORWARD_ENV.iter().filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v))).collect();
    // About as many rows as the pane the tool runs in has: used to center the popup vertically.
    let rows = ratatui::crossterm::terminal::size().ok().map(|(_, r)| r);
    let zargs = zellij_args(&exe, &args, &cwd, has_input.then_some(input.as_path()), &out, &code, percent, rows, &env);

    let status = Command::new("zellij").args(&zargs).stdout(std::process::Stdio::null()).status()?;
    if !status.success() {
        return Err(io::Error::other(format!("zellij exited with {status}")));
    }

    if let Ok(selection) = std::fs::read(&out) {
        io::stdout().write_all(&selection)?;
    }
    // No code file: the pane was closed before `pick` finished, which is a cancel.
    Ok(std::fs::read_to_string(&code).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(130))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_round_trips_through_sh() {
        for s in ["plain", "with space", "it's", "a\"b$c`d", "new\nline", "", "--flag=1"] {
            let out = Command::new("sh").arg("-c").arg(format!("printf %s {}", quote(s))).output().unwrap();
            assert_eq!(String::from_utf8_lossy(&out.stdout), s, "{s:?}");
        }
    }

    #[test]
    fn the_pane_command_is_a_centered_floating_blocking_pane() {
        let args: Vec<OsString> = vec!["--files".into(), "my dir".into(), "--title".into(), "it's".into()];
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
        assert!(z.contains(&"--floating".to_string()) && z.contains(&"--blocking".to_string()) && z.contains(&"--close-on-exit".to_string()));
        assert!(has("--borderless", "true"));
        assert!(has("--width", "80%") && has("--height", "80%") && has("--x", "10%") && has("--y", "10%") && has("--cwd", "/work"));
        let script = z.last().unwrap();
        assert_eq!(
            script,
            "PATH='/a:/b' '/bin/pick' '--files' 'my dir' '--title' 'it'\\''s' --no-popup < '/t/in' > '/t/out'; echo $? > '/t/code'"
        );
        // No piped input: no redirect from `in`. Other sizes are centered too.
        let z = zellij_args(Path::new("/bin/pick"), &[], Path::new("/w"), None, Path::new("/o"), Path::new("/c"), 60, None, &[]);
        assert!(!z.last().unwrap().contains(" < ") && z.windows(2).any(|w| w[0] == "--x" && w[1] == "20%"));
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
                assert!((top - bottom).abs() <= 2, "rows {rows} p {p}: y {y} top {top} bottom {bottom}");
            }
        }
        // Full height needs no offset.
        assert_eq!(vertical_offset(100, 40), 0);
    }
}
