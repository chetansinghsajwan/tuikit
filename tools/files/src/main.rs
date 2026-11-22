//! `files`: the standalone file explorer (tree, preview pane, tabs) on the same
//! panel the editors embed.

/// The explorer itself: terminal setup, tabs, preview and the actions around the shared panel.
mod app;
/// The default keys of the explorer's own actions.
mod binds;
/// The standalone explorer's own actions.
mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

/// File explorer. Prints nothing; use `--chooser-file` to pick files for another
/// program and `--cwd-file` to let the shell follow where you ended up.
#[derive(Parser)]
#[command(version, args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
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

/// What `files` does besides the explorer.
#[derive(Subcommand)]
enum Command {
    /// Print a folder's entries as a list (permissions, size, age, git state) and exit.
    List {
        /// Folder to list (default: current directory).
        dir: Option<PathBuf>,
        /// Disable git status colors and signs.
        #[arg(long)]
        no_git: bool,
    },
}

/// Prints the list of `dir` to stdout, as wide as `$w` or the terminal (100 when neither is known).
fn list(dir: PathBuf, git: bool) -> Result<u8, String> {
    let width = std::env::var("w")
        .ok()
        .and_then(|w| w.parse().ok())
        .or_else(|| ratatui::crossterm::terminal::size().ok().map(|(w, _)| w))
        .unwrap_or(100);
    let opts = pv::Options { width: Some(width), height: None, git, focus: None };
    let lines = pv::list_dir(&dir, &ui::Theme::load(), &opts)
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    print!("{}", pv::to_ansi(&lines));
    Ok(0)
}

/// Loads the theme config, parses the command line and runs the app. Failures exit with code 2.
fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Some(Command::List { dir, no_git }) = cli.command {
        return ui::app::main("files", || list(dir.unwrap_or_else(|| PathBuf::from(".")), !no_git));
    }
    ui::app::main("files", || app::run(cli.into()).map(|()| 0))
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
