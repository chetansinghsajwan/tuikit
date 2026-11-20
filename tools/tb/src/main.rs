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
    ui::app::main("tb", || {
        let cli = Cli::parse();
        let mut grid = Grid::open(&cli.file).map_err(|e| e.to_string())?;
        ui::app::Shell::new("tb")
            .actions(tb::keys::ACTIONS)
            .groups(tb::keys::GROUPS)
            .run(&mut grid)
            .map(|()| 0)
            .map_err(|e| e.to_string())
    })
}
