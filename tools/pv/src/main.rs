//! `pv <path>`: prints a colored preview of a file to stdout, for file managers and shell use.
//! `pv --name <file> -` previews standard input as that file would look (the name only picks the
//! syntax), and `pv --diff` draws a unified diff from standard input (a git pager).

use pv::{Config, Options, preview, preview_text, preview_unified, to_ansi};
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str =
    "usage: pv [--no-git] [--raw] <path>\n       pv [--name <file>] -\n       pv --diff";

/// Reads a number from an environment variable, or `None` when unset or not a number.
fn env_num(name: &str) -> Option<u16> {
    std::env::var(name).ok()?.parse().ok()
}

/// The room for a preview: `$w` and `$h` (a preview pane), else, for a diff, the terminal's width (its
/// output is a pipe to a pager).
fn size(terminal_width: bool) -> (Option<u16>, Option<u16>) {
    let width = env_num("w").or_else(|| {
        terminal_width.then(|| ratatui::crossterm::terminal::size().ok().map(|(w, _)| w)).flatten()
    });
    (width, env_num("h"))
}

fn read_stdin() -> Vec<u8> {
    let mut buf = Vec::new();
    // A read error leaves what came in so far, which is still worth drawing.
    let _ = std::io::stdin().lock().read_to_end(&mut buf);
    buf
}

/// Reads the flags and the path, renders the preview and prints it as ANSI text. Exit code 1 on
/// a usage error or when the preview fails.
fn main() -> ExitCode {
    let mut git = Config::load().git;
    let mut diff = false;
    let mut name: Option<PathBuf> = None;
    let mut paths = Vec::new();
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        if a == "--no-git" {
            git = false;
        } else if a == "--raw" {
            pv::set_raw_tables(true);
        } else if a == "--diff" {
            diff = true;
        } else if a == "--name" {
            let Some(n) = args.next() else {
                eprintln!("{USAGE}");
                return ExitCode::from(1);
            };
            name = Some(PathBuf::from(n));
        } else {
            paths.push(PathBuf::from(a));
        }
    }
    let theme = ui::Theme::load();
    let cx = &ui::Context::new(&theme);

    if diff {
        let (width, height) = size(true);
        let opts = Options { width, height, git, focus: None };
        let text = String::from_utf8_lossy(&read_stdin()).into_owned();
        print!("{}", to_ansi(&preview_unified(&text, cx, &opts)));
        return ExitCode::SUCCESS;
    }

    let [path] = paths.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(1);
    };
    if path.as_os_str() == "-" {
        let (width, height) = size(false);
        let opts = Options { width, height, git: false, focus: None };
        let name = name.unwrap_or_else(|| PathBuf::from("stdin"));
        return match preview_text(&name, &read_stdin(), cx, &opts) {
            Ok(lines) => {
                print!("{}", to_ansi(&lines));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("pv: stdin: {e}");
                ExitCode::from(1)
            }
        };
    }

    // yazi's piper sets $w and $h to the preview pane size.
    let (width, height) = size(false);
    let opts = Options { width, height, git, focus: None };
    match preview(path, cx, &opts) {
        Ok(lines) => {
            print!("{}", to_ansi(&lines));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("pv: {}: {e}", path.display());
            ExitCode::from(1)
        }
    }
}
