//! `sm`: the system monitor.

use clap::Parser;
use std::process::ExitCode;
use std::time::Duration;

/// System monitor: CPU, memory, disks, network and processes.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Seconds between readings.
    #[arg(short, long, default_value_t = 1.0, value_parser = parse_interval)]
    interval: f64,
}

/// An interval of at least a quarter second: CPU use needs time between two readings to mean anything.
fn parse_interval(text: &str) -> Result<f64, String> {
    let value: f64 = text.parse().map_err(|_| format!("{text} is not a number"))?;
    if value.is_finite() && value >= 0.25 {
        Ok(value)
    } else {
        Err("the interval must be at least 0.25 seconds".into())
    }
}

/// Opens the monitor and runs it. Failures exit with code 2.
fn main() -> ExitCode {
    ui::app::main("sm", || {
        let cli = Cli::parse();
        let mut monitor = sm::monitor::Monitor::new()?;
        ui::app::Shell::new("sm")
            .commands(sm::keys::commands())
            .groups(sm::keys::GROUPS)
            .tick_interval(Duration::from_secs_f64(cli.interval))
            .run(&mut monitor)
            .map(|()| 0)
            .map_err(|e| e.to_string())
    })
}
