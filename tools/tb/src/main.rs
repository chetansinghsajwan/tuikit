//! `tb`: the table editor (csv, tsv, psv).

use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;
use tb::grid::Grid;

/// Table editor for csv, tsv and psv files.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// The file to edit (created when saved if it does not exist).
    file: PathBuf,
}

/// Opens the table file and runs the editor. Failures exit with code 2.
fn main() -> ExitCode {
    for p in ui::config::init("tb") {
        eprintln!("tb: {p}");
    }
    let cli = Cli::parse();
    let mut grid = match Grid::open(&cli.file) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("tb: {e}");
            return ExitCode::from(2);
        }
    };
    match ui::app::Shell::new("tb")
        .actions(tb::keys::ACTIONS)
        .groups(tb::keys::GROUPS)
        .run(&mut grid)
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tb: {e}");
            ExitCode::from(2)
        }
    }
}
