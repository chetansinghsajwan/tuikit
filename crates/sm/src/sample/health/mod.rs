//! The health of the disks (SMART): whether a drive reports itself failing. It asks an outside tool
//! (`smartctl` on Linux, which usually needs root, `diskutil` on macOS), so it is read once, in the
//! background; a disk whose health cannot be read shows none.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// What a drive says about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Good,
    Failing,
}

/// The health of the drives, filled in by a background thread.
#[derive(Debug, Clone, Default)]
pub struct Watch {
    found: Arc<Mutex<HashMap<String, Health>>>,
}

impl Watch {
    /// Starts reading the health of `drives` (whole-disk device names, see `whole_disk`).
    pub fn spawn(drives: Vec<String>) -> Watch {
        let watch = Watch::default();
        let found = watch.found.clone();
        std::thread::spawn(move || {
            for drive in drives {
                if let Some(health) = read(&drive)
                    && let Ok(mut map) = found.lock()
                {
                    map.insert(drive, health);
                }
            }
        });
        watch
    }

    /// The health of `drive`, once read; `None` while reading or when it cannot be read.
    pub fn get(&self, drive: &str) -> Option<Health> {
        self.found.lock().ok()?.get(drive).copied()
    }
}

/// The whole disk a partition belongs to: `/dev/nvme0n1p2` and `/dev/sda1` on Linux become
/// `/dev/nvme0n1` and `/dev/sda`, `disk3s1s1` on macOS becomes `disk3`. Other names stay as they are.
pub fn whole_disk(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("disk") {
        let n: String = rest.chars().take_while(char::is_ascii_digit).collect();
        return format!("disk{n}");
    }
    for prefix in ["/dev/nvme", "/dev/mmcblk"] {
        if name.starts_with(prefix)
            && let Some(at) = name.rfind('p')
            && at > prefix.len()
            && name[at + 1..].chars().all(|c| c.is_ascii_digit())
            && at + 1 < name.len()
        {
            return name[..at].to_string();
        }
    }
    if name.starts_with("/dev/sd") || name.starts_with("/dev/vd") || name.starts_with("/dev/hd") {
        return name.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
    }
    name.to_string()
}

/// The health of `drive`, asking the platform's tool; `None` when it cannot tell.
fn read(drive: &str) -> Option<Health> {
    let run = |cmd: &str, args: &[&str]| {
        let out = std::process::Command::new(cmd).args(args).output().ok()?;
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    if cfg!(target_os = "macos") {
        parse_diskutil(&run("diskutil", &["info", drive])?)
    } else {
        parse_smartctl(&run("smartctl", &["-H", "--json=c", drive])?)
    }
}

/// The health in `smartctl -H --json` output (`smart_status.passed`).
pub fn parse_smartctl(json: &str) -> Option<Health> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    match value.get("smart_status")?.get("passed")?.as_bool()? {
        true => Some(Health::Good),
        false => Some(Health::Failing),
    }
}

/// The health in `diskutil info` output (`SMART Status: Verified` or `Failing`).
pub fn parse_diskutil(text: &str) -> Option<Health> {
    let status = text.lines().find_map(|l| l.trim().strip_prefix("SMART Status:"))?.trim();
    match status {
        "Verified" => Some(Health::Good),
        "Failing" => Some(Health::Failing),
        _ => None,
    }
}

/// Tests for the disk health.
#[cfg(test)]
mod tests;
