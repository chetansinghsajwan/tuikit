//! `tb`: the table editor (csv, tsv, psv).

use clap::Parser;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use std::fs::OpenOptions;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use tb::grid::{Action, Grid};
use ui::Theme;

/// Table editor for csv, tsv and psv files.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// The file to edit (created when saved if it does not exist).
    file: PathBuf,
}

type Term = Terminal<CrosstermBackend<std::fs::File>>;

fn main() -> ExitCode {
    for p in ui::config::init("tb") {
        eprintln!("tb: {p}");
    }
    let cli = Cli::parse();
    let grid = match Grid::open(&cli.file, "space") {
        Ok(g) => g,
        Err(e) => {
            eprintln!("tb: {e}");
            return ExitCode::from(2);
        }
    };
    match run(grid) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tb: {e}");
            ExitCode::from(2)
        }
    }
}

fn run(mut grid: Grid) -> io::Result<()> {
    // Draw on the controlling terminal so stdout stays free.
    let tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
    let mut term: Term = Terminal::new(CrosstermBackend::new(tty))?;
    enable_raw_mode()?;
    execute!(term.backend_mut(), EnterAlternateScreen, Clear(ClearType::All))?;
    term.clear()?;
    let result = event_loop(&mut term, &mut grid);
    let _ = disable_raw_mode();
    let _ = execute!(term.backend_mut(), LeaveAlternateScreen);
    result
}

fn event_loop(term: &mut Term, grid: &mut Grid) -> io::Result<()> {
    let theme = Theme::load();
    let mut help: Option<ui::keys::Help> = None;
    loop {
        term.draw(|f| {
            let area = f.area();
            grid.render(area, f.buffer_mut(), &theme, true);
            if let Some(menu) = grid.menu() {
                ui::keys::render_menu(area, f.buffer_mut(), &theme, &menu);
            }
            if let Some(h) = help.as_mut() {
                let rows = grid.keys_rows();
                h.render(area, f.buffer_mut(), &theme, "Keys", &rows);
            }
            ui::anim::finish(f.buffer_mut(), &theme);
        })?;
        if !event::poll(ui::anim::poll_timeout(Duration::from_millis(250)))? {
            continue;
        }
        let ev = event::read()?;
        if let (Some(h), Event::Key(key)) = (help.as_mut(), &ev) {
            if !h.key(key) {
                help = None;
            }
            continue;
        }
        match grid.handle_event(&ev) {
            Action::Quit => return Ok(()),
            Action::Help => help = Some(ui::keys::Help::default()),
            Action::Palette => {
                let rows = grid.keys_rows();
                let picked = if ui::popup::wanted(false) {
                    ui::keys::choose("tb", &rows, "Commands")
                } else {
                    let _ = disable_raw_mode();
                    let _ = execute!(term.backend_mut(), LeaveAlternateScreen);
                    let id = ui::keys::choose("tb", &rows, "Commands");
                    enable_raw_mode()?;
                    execute!(term.backend_mut(), EnterAlternateScreen, Clear(ClearType::All))?;
                    term.clear()?;
                    id
                };
                if let Some(id) = picked {
                    grid.run(id);
                }
            }
            Action::None => {}
        }
    }
}
