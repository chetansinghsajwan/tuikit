use clap::Parser;
use nu::{App, Quit};
use nu_core::Backend;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use std::fs::OpenOptions;
use std::io;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

/// Connections on one screen: Wi-Fi (more kinds to come), through NetworkManager.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Use a pretend radio with a few networks instead of NetworkManager (to try the screen).
    #[arg(long)]
    fake: bool,
}

type Term = Terminal<CrosstermBackend<std::fs::File>>;

fn enter(term: &mut Term) -> io::Result<()> {
    enable_raw_mode()?;
    execute!(term.backend_mut(), EnterAlternateScreen, Clear(ClearType::All))?;
    term.clear()
}

fn leave(term: &mut Term) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("nu: {e}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> io::Result<u8> {
    let backend: Arc<dyn Backend> = if cli.fake {
        Arc::new(nu_core::fake::Fake::new())
    } else {
        match nu_core::nm::Nm::system() {
            Ok(nm) => Arc::new(nm),
            Err(e) => {
                eprintln!("nu: {e} (is NetworkManager running? `nu --fake` shows the screen without it)");
                return Ok(2);
            }
        }
    };
    let mut app = App::new(backend);
    let tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
    let mut term: Term = Terminal::new(CrosstermBackend::new(tty))?;
    enter(&mut term)?;
    let result = (|| -> io::Result<()> {
        let mut dirty = true;
        loop {
            if dirty {
                term.draw(|f| {
                    let area = f.area();
                    app.render(area, f.buffer_mut());
                })?;
            }
            if !event::poll(Duration::from_millis(200))? {
                dirty = app.tick();
                continue;
            }
            let ev = event::read()?;
            dirty = true;
            if matches!(ev, Event::Resize(..)) {
                continue;
            }
            let mut quit = app.handle_event(&ev);
            // The palette: inside zellij `pick` is a floating pane over this screen; elsewhere it
            // takes the terminal for a moment.
            while quit == Quit::Palette {
                let popup = ui::popup::wanted(false);
                if !popup {
                    leave(&mut term)?;
                }
                let picked = ui::keys::choose(&app.keys_rows(), "Commands");
                if !popup {
                    enter(&mut term)?;
                }
                quit = match picked {
                    Some(id) => app.run_palette(id),
                    None => Quit::No,
                };
            }
            if quit == Quit::Yes {
                return Ok(());
            }
        }
    })();
    let _ = leave(&mut term);
    result?;
    Ok(0)
}
