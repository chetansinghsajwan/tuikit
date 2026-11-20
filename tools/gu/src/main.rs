//! The `gu` binary: opens the repository and runs `gu::App` on the shared app shell.

use clap::Parser;
use gu::App;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

/// A multi-pane git screen: files, branches, commits and stashes with their diffs.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// A path inside the repository (default: the current directory).
    dir: Option<PathBuf>,
}

/// Start-up and exit code: 0 on a normal quit, 2 on an error or when `dir` is no repository.
fn main() -> ExitCode {
    for p in ui::config::init("gu") {
        eprintln!("gu: {p}");
    }
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("gu: {e}");
            ExitCode::from(2)
        }
    }
}

/// Open the repository and run the screen on the shell until it asks to quit.
fn run(cli: Cli) -> io::Result<u8> {
    let dir = cli.dir.map(Ok).unwrap_or_else(std::env::current_dir)?;
    let Some(mut app) = App::new(&dir) else {
        eprintln!("gu: not a git repository: {}", dir.display());
        return Ok(2);
    };
    ui::app::Shell::new("gu")
        .contexts(gu::contexts())
        .groups(gu::GROUPS)
        .tick_interval(Duration::from_millis(200))
        .run(&mut app)?;
    Ok(0)
}
