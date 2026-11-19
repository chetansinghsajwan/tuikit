//! The `nu` binary: picks the real or fake radios and runs `nu::App` on the shared app shell.

use clap::Parser;
use nu::App;
use nu_core::Backend;
use nu_core::bluetooth::Bluetooth;
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

/// Loads the shared config, runs the app and turns its result into an exit code.
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
        Err(e) => {
            Arc::new(nu_core::Unavailable(format!("{e} (try `nu --fake` to see the screen)")))
        }
    };
    let bt: Arc<dyn Bluetooth> = match nu_core::bluez::BlueZ::system() {
        Ok(b) => Arc::new(b),
        Err(e) => Arc::new(nu_core::Unavailable(e.to_string())),
    };
    (wifi, bt)
}

/// The real radios on macOS: CoreWLAN for Wi-Fi and IOBluetooth for Bluetooth.
#[cfg(target_os = "macos")]
fn real_backends() -> (Arc<dyn Backend>, Arc<dyn Bluetooth>) {
    (Arc::new(nu_core::macos::MacWifi::new()), Arc::new(nu_core::macos::MacBluetooth::new()))
}

/// Sets up the backends and runs the app on the shell. Returns the exit code.
fn run(cli: Cli) -> io::Result<u8> {
    // Each radio that cannot be reached shows why in its own pane.
    let (wifi, bt): (Arc<dyn Backend>, Arc<dyn Bluetooth>) = if cli.fake {
        (Arc::new(nu_core::fake::Fake::new()), Arc::new(nu_core::fake::FakeBluetooth::new()))
    } else {
        real_backends()
    };
    let mut app = App::new(wifi, bt);
    ui::app::App::new("nu")
        .actions(nu::common())
        .contexts(nu::panes())
        .tick(Duration::from_millis(200))
        .run(&mut app)?;
    Ok(0)
}
