//! `sm`: the system monitor.

use clap::Parser;
use sm::panels::Panel;
use std::process::ExitCode;
use std::time::Duration;

/// System monitor: CPU, memory, disks, network and processes.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Milliseconds between readings.
    #[arg(short, long, default_value_t = 1000, value_name = "MS", value_parser = parse_interval)]
    interval: u64,

    /// Panels that start hidden: cpu, memory, network, processes, sensors (1 to 5 toggle them while running).
    #[arg(long, value_delimiter = ',', value_parser = parse_panel)]
    hide: Vec<Panel>,
}

/// An interval of at least 250 ms: CPU use needs time between two readings to mean anything.
fn parse_interval(text: &str) -> Result<u64, String> {
    let value: u64 = text.parse().map_err(|_| format!("{text} is not a number of milliseconds"))?;
    if value >= 250 { Ok(value) } else { Err("the interval must be at least 250 ms".into()) }
}

/// A panel by name.
fn parse_panel(text: &str) -> Result<Panel, String> {
    Panel::parse(text)
        .ok_or_else(|| format!("{text} is not a panel (cpu, memory, network, processes, sensors)"))
}

/// Opens the monitor and runs it. Failures exit with code 2.
fn main() -> ExitCode {
    ui::app::main("sm", || {
        let cli = Cli::parse();
        let interval = Duration::from_millis(cli.interval);
        let mut monitor = sm::monitor::Monitor::new(interval, &cli.hide)?;
        ui::app::Shell::new("sm")
            .commands(sm::keys::commands())
            .groups(sm::keys::GROUPS)
            .tick_interval(interval)
            .run(&mut monitor)
            .map(|()| 0)
            .map_err(|e| e.to_string())
    })
}
