//! `pv <path>`: prints a colored preview of a file to stdout, for file managers and shell use.

use pv::{Config, Options, preview, to_ansi};
use std::path::PathBuf;
use std::process::ExitCode;

/// Reads a number from an environment variable, or `None` when unset or not a number.
fn env_num(name: &str) -> Option<u16> {
    std::env::var(name).ok()?.parse().ok()
}

/// Reads the flags and the path, renders the preview and prints it as ANSI text. Exit code 1 on
/// a usage error or when the preview fails.
fn main() -> ExitCode {
    let mut git = Config::load().git;
    let mut paths = Vec::new();
    for a in std::env::args_os().skip(1) {
        if a == "--no-git" {
            git = false;
        } else if a == "--raw" {
            pv::set_raw_tables(true);
        } else {
            paths.push(PathBuf::from(a));
        }
    }
    let [path] = paths.as_slice() else {
        eprintln!("usage: pv [--no-git] [--raw] <path>");
        return ExitCode::from(1);
    };
    // yazi's piper sets $w and $h to the preview pane size.
    let opts = Options { width: env_num("w"), height: env_num("h"), git, focus: None };
    let theme = ui::Theme::load();
    match preview(path, &theme, &opts) {
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
