use clap::Parser;
use nu::{App, Quit};
use nu_core::Backend;
use nu_core::bluetooth::Bluetooth;
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

/// Connections on one screen: Wi-Fi through NetworkManager and Bluetooth through BlueZ.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Use pretend radios (a few networks and devices) instead of NetworkManager and BlueZ, to try the screen.
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
    for p in ui::config::init("nu") {
        eprintln!("nu: {p}");
    }
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("nu: {e}");
            ExitCode::from(2)
        }
    }
}

/// The real radios of this system: NetworkManager and BlueZ on Linux, CoreWLAN and IOBluetooth on
/// macOS. One that cannot be reached shows why in its own pane.
#[cfg(not(target_os = "macos"))]
fn real_backends() -> (Arc<dyn Backend>, Arc<dyn Bluetooth>) {
    let wifi: Arc<dyn Backend> = match nu_core::nm::Nm::system() {
        Ok(nm) => Arc::new(nm),
        Err(e) => Arc::new(nu_core::Unavailable(format!("{e} (try `nu --fake` to see the screen)"))),
    };
    let bt: Arc<dyn Bluetooth> = match nu_core::bluez::BlueZ::system() {
        Ok(b) => Arc::new(b),
        Err(e) => Arc::new(nu_core::Unavailable(e.to_string())),
    };
    (wifi, bt)
}

#[cfg(target_os = "macos")]
fn real_backends() -> (Arc<dyn Backend>, Arc<dyn Bluetooth>) {
    (Arc::new(nu_core::macos::MacWifi::new()), Arc::new(nu_core::macos::MacBluetooth::new()))
}

fn run(cli: Cli) -> io::Result<u8> {
    // Each radio that cannot be reached shows why in its own pane.
    let (wifi, bt): (Arc<dyn Backend>, Arc<dyn Bluetooth>) = if cli.fake {
        (Arc::new(nu_core::fake::Fake::new()), Arc::new(nu_core::fake::FakeBluetooth::new()))
    } else {
        real_backends()
    };
    let mut app = App::new(wifi, bt);
    let tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
    let mut term: Term = Terminal::new(CrosstermBackend::new(tty))?;
    enter(&mut term)?;
    let result = (|| -> io::Result<()> {
        let theme = ui::Theme::load();
        let mut dirty = true;
        loop {
            if dirty {
                term.draw(|f| {
                    let area = f.area();
                    app.render(area, f.buffer_mut());
                    ui::anim::finish(f.buffer_mut(), &theme);
                })?;
            }
            if !event::poll(ui::anim::poll_timeout(Duration::from_millis(200)))? {
                dirty = app.tick() | ui::anim::take_redraw();
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
                let picked = ui::keys::choose("nu", &app.keys_rows(), "Commands");
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
