//! The `cb` binary: a clipboard history. `cb watch` records every text copy, `cb` browses the
//! history and puts the chosen entry back on the clipboard. It is its own tool, with its own
//! store, so it works wherever a clipboard backend does (Wayland, WSLg, X11, macOS) and does not
//! depend on a desktop session starting a recorder.

mod backend;
mod browse;
mod record;
mod store;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

/// Browse the clipboard history; Enter copies the entry, ctrl-x deletes it.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
    /// Initial query.
    #[arg(long, short, default_value = "")]
    query: String,
}

/// What `cb` can do besides browsing.
#[derive(Subcommand)]
enum Cmd {
    /// Record every text copy until stopped (run it as a service).
    Watch,
    /// Record the text on stdin as the newest entry.
    Store,
}

/// Exit code for a failure other than a cancel.
const EXIT_ERROR: u8 = 2;

/// Parses the command line and runs the chosen command.
fn main() -> ExitCode {
    let cli = Cli::parse();
    let backend = backend::detect();
    let result = match cli.command {
        Some(Cmd::Watch) => record::watch(backend.as_ref()),
        Some(Cmd::Store) => record::store(),
        None => browse::run(cli.query, backend.as_ref()),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("cb: {e}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}
