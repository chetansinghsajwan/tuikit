//! Sizes, rates and durations as short text, so every pane of the monitor writes them alike.

/// A byte count with a binary unit and at most one decimal: `0B`, `1.5K`, `12M`, `3.2G`.
pub fn bytes(count: u64) -> String {
    const UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];
    let mut value = count as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 || value >= 10.0 {
        format!("{value:.0}{}", UNITS[unit])
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

/// A transfer rate in bytes per second: `1.5M/s`.
pub fn rate(bytes_per_second: u64) -> String {
    format!("{}/s", bytes(bytes_per_second))
}

/// A duration as `1d 2h`, `3h 4m` or `5m 6s`: the two largest units that are not zero.
pub fn duration(seconds: u64) -> String {
    let (d, h, m, s) =
        (seconds / 86_400, seconds % 86_400 / 3600, seconds % 3600 / 60, seconds % 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m {s}s")
    }
}

/// Tests for the formats.
#[cfg(test)]
mod tests;
