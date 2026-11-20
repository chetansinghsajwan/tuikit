//! The `kanata-status` binary: wires the config and the daemon link into the app shell.

use clap::Parser;
use kanata_status::kbd::Config;
use kanata_status::{ACTIONS, App, link};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Duration;

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

/// Finds the kanata config when `--cfg` is not given, honouring `$KANATA_CFG` and XDG.
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

/// Entry point: loads the theme config, runs the app and maps a failure to exit code 2.
fn main() -> ExitCode {
    ui::app::main("kanata-status", || run(Cli::parse()).map(|()| 0))
}

/// Parses the config, follows the daemon and runs the screen on the shared app shell, which
/// restores the terminal on the way out, even after an error.
fn run(cli: Cli) -> Result<(), String> {
    let path = cli.cfg.unwrap_or_else(default_cfg);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let cfg = Config::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let (tx, rx) = mpsc::channel();
    link::follow(cli.addr.clone(), move |e| tx.send(e).is_ok());
    let mut app = App::new(cfg, cli.addr).with_events(rx);
    ui::app::Shell::new("kanata-status")
        .leader("ctrl-space")
        .with_navigation(false)
        .actions(ACTIONS)
        .tick_interval(Duration::from_millis(100))
        .run(&mut app)
        .map_err(|e| e.to_string())
}
