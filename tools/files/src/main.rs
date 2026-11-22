//! `files`: the standalone file explorer (tree, preview pane, tabs) on the same
//! panel the editors embed.

/// The explorer itself: terminal setup, tabs, preview and the actions around the shared panel.
mod app;
/// The default keys of the explorer's own actions.
mod binds;
/// The standalone explorer's own actions.
mod commands;

use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;

/// File explorer. Prints nothing; use `--chooser-file` to pick files for another
/// program and `--cwd-file` to let the shell follow where you ended up.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Folder to start in (default: current directory).
    dir: Option<PathBuf>,
    /// Write the chosen file's path to this file and exit when one is opened.
    #[arg(long, value_name = "FILE")]
    chooser_file: Option<PathBuf>,
    /// On exit, write the folder the explorer was in to this file (for a shell `cd`).
    #[arg(long, value_name = "FILE")]
    cwd_file: Option<PathBuf>,
    /// Start without the hidden (dot) files; they are shown by default. `.` toggles them.
    #[arg(long)]
    no_hidden: bool,
    /// No preview pane.
    #[arg(long)]
    no_preview: bool,
    /// Disable git status colors and signs.
    #[arg(long)]
    no_git: bool,
}

/// Loads the theme config, parses the command line and runs the app. Failures exit with code 2.
fn main() -> ExitCode {
    ui::app::main("files", || app::run(Cli::parse().into()).map(|()| 0))
}

impl From<Cli> for app::Options {
    /// Turns the parsed command line into the app's options, applying the defaults.
    fn from(c: Cli) -> Self {
        app::Options {
            dir: c.dir.unwrap_or_else(|| PathBuf::from(".")),
            chooser_file: c.chooser_file,
            cwd_file: c.cwd_file,
            hidden: !c.no_hidden,
            preview: !c.no_preview,
            git: !c.no_git,
        }
    }
}
