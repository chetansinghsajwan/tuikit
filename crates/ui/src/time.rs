//! Times as people read them.

/// How long ago `time` (seconds since the epoch) was: `now`, `5m ago`, `3h ago`, `2d ago`, `4mo ago`, `1y ago`.
pub fn ago(time: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(time);
    match (now - time).max(0) {
        0..60 => "now".into(),
        s @ 60..3600 => format!("{}m ago", s / 60),
        s @ 3600..86_400 => format!("{}h ago", s / 3600),
        s @ 86_400..2_592_000 => format!("{}d ago", s / 86_400),
        s @ 2_592_000..31_536_000 => format!("{}mo ago", s / 2_592_000),
        s => format!("{}y ago", s / 31_536_000),
    }
}

#[cfg(test)]
mod tests;
