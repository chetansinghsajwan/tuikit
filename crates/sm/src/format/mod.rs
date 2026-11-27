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

/// The tops a network graph can have, in bytes per second: 5K, 10K, 50K, 100K, 500K, 1M, ... 10G.
pub const RATE_STEPS: [f32; 14] = {
    const K: f32 = 1024.0;
    const M: f32 = K * K;
    const G: f32 = M * K;
    [
        5.0 * K,
        10.0 * K,
        50.0 * K,
        100.0 * K,
        500.0 * K,
        M,
        5.0 * M,
        10.0 * M,
        50.0 * M,
        100.0 * M,
        500.0 * M,
        G,
        5.0 * G,
        10.0 * G,
    ]
};

/// A rate for the label of a scale: `5K/s`, `1M/s`, `2.5K/s` (a round number has no `.0`).
pub fn rate_label(bytes_per_second: f32) -> String {
    rate(bytes_per_second as u64).replacen(".0", "", 1)
}

/// How hot a transfer rate is, 0 to 1, on a log scale from 1K/s (0) to 1G/s (1): a disk or a
/// network rate has no ceiling, and each tenfold is a step of the same size.
pub fn rate_load(bytes_per_second: u64) -> f32 {
    if bytes_per_second == 0 {
        return 0.0;
    }
    (((bytes_per_second as f32).log10() - 3.0) / 6.0).clamp(0.0, 1.0)
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
