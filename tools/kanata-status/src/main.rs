use clap::Parser;
use kanata_status::kbd::Config;
use kanata_status::{App, link};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use std::fs::OpenOptions;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;
use std::thread;

/// The keyboard, the active kanata layer, the keys held and the keys pressed.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// The kanata config to draw (default: $KANATA_CFG, else ~/.config/kanata/kanata.kbd).
    #[arg(long)]
    cfg: Option<PathBuf>,
    /// Where `kb daemon` serves its state.
    #[arg(long, default_value = "127.0.0.1:6667")]
    addr: String,
}

/// Everything the main loop waits on, so it sleeps until something happens.
enum Msg {
    Daemon(link::Event),
    Term(event::Event),
}

type Term = Terminal<CrosstermBackend<std::fs::File>>;

fn default_cfg() -> PathBuf {
    if let Some(p) = std::env::var_os("KANATA_CFG") {
        return p.into();
    }
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_default();
    config.join("kanata/kanata.kbd")
}

fn main() -> ExitCode {
    for p in ui::config::init("kanata-status") {
        eprintln!("kanata-status: {p}");
    }
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kanata-status: {e}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let path = cli.cfg.unwrap_or_else(default_cfg);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let cfg = Config::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut app = App::new(cfg, cli.addr.clone());
    let (tx, rx) = mpsc::channel();
    let daemon_tx = tx.clone();
    link::follow(cli.addr, move |e| daemon_tx.send(Msg::Daemon(e)).is_ok());
    thread::spawn(move || {
        while let Ok(e) = event::read() {
            if tx.send(Msg::Term(e)).is_err() {
                break;
            }
        }
    });

    let tty =
        OpenOptions::new().read(true).write(true).open("/dev/tty").map_err(|e| e.to_string())?;
    let mut term: Term = Terminal::new(CrosstermBackend::new(tty)).map_err(|e| e.to_string())?;
    enable_raw_mode().map_err(|e| e.to_string())?;
    execute!(term.backend_mut(), EnterAlternateScreen, Clear(ClearType::All))
        .map_err(|e| e.to_string())?;
    let result = (|| -> io::Result<()> {
        let mut dirty = true;
        loop {
            if dirty {
                term.draw(|f| {
                    let area = f.area();
                    app.render(area, f.buffer_mut());
                })?;
                dirty = false;
            }
            let Ok(first) = rx.recv() else { return Ok(()) };
            let mut next = Some(first);
            while let Some(msg) = next {
                match msg {
                    Msg::Daemon(e) => dirty |= app.apply(e),
                    Msg::Term(e) => {
                        let (quit, redraw) = app.handle_event(&e);
                        if quit {
                            return Ok(());
                        }
                        dirty |= redraw;
                    }
                }
                next = rx.try_recv().ok();
            }
        }
    })();
    let _ = disable_raw_mode();
    let _ = execute!(term.backend_mut(), LeaveAlternateScreen);
    result.map_err(|e| e.to_string())
}
