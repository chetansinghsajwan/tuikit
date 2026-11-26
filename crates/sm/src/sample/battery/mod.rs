//! The battery: how full it is, whether it charges, and how long it lasts. `sysinfo` does not read
//! it, so this reads the platform: `/sys/class/power_supply` on Linux, `pmset -g batt` on macOS.

/// What the battery does now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Charge {
    Charging,
    Discharging,
    Full,
    /// Plugged in but neither charging nor full (held at a limit).
    Holding,
}

impl Charge {
    /// The word shown for it.
    pub fn label(self) -> &'static str {
        match self {
            Charge::Charging => "charging",
            Charge::Discharging => "on battery",
            Charge::Full => "full",
            Charge::Holding => "plugged in",
        }
    }
}

/// The battery at one moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Battery {
    /// How full, 0 to 100.
    pub percent: f32,
    pub charge: Charge,
    /// Seconds until empty (discharging) or full (charging), when the platform says.
    pub time_left: Option<u64>,
}

/// The battery of this machine, or `None` when it has none (or it cannot be read).
pub fn read() -> Option<Battery> {
    #[cfg(target_os = "linux")]
    {
        linux::read(std::path::Path::new("/sys/class/power_supply"))
    }
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("pmset").args(["-g", "batt"]).output().ok()?;
        parse_pmset(&String::from_utf8_lossy(&out.stdout))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// Reading the battery from sysfs.
#[cfg(target_os = "linux")]
pub(super) mod linux {
    use super::{Battery, Charge};
    use std::path::Path;

    /// The first battery under `root` (a `power_supply` folder).
    pub fn read(root: &Path) -> Option<Battery> {
        let mut entries: Vec<_> =
            std::fs::read_dir(root).ok()?.flatten().map(|e| e.path()).collect();
        entries.sort();
        entries.into_iter().find_map(|dir| {
            let field = |name: &str| {
                std::fs::read_to_string(dir.join(name)).ok().map(|s| s.trim().to_string())
            };
            if field("type")? != "Battery" {
                return None;
            }
            let percent: f32 = field("capacity")?.parse().ok()?;
            let charge = match field("status").as_deref() {
                Some("Charging") => Charge::Charging,
                Some("Discharging") => Charge::Discharging,
                Some("Full") => Charge::Full,
                _ => Charge::Holding,
            };
            let number = |name: &str| field(name).and_then(|v| v.parse::<f64>().ok());
            // Energy in µWh and power in µW, or charge in µAh and current in µA: the ratio is hours.
            let (now, full, rate) = match (number("energy_now"), number("power_now")) {
                (Some(now), Some(rate)) => (now, number("energy_full"), rate),
                _ => (number("charge_now")?, number("charge_full"), number("current_now")?),
            };
            let hours = match charge {
                Charge::Discharging if rate > 0.0 => Some(now / rate),
                Charge::Charging if rate > 0.0 => full.map(|f| (f - now).max(0.0) / rate),
                _ => None,
            };
            Some(Battery { percent, charge, time_left: hours.map(|h| (h * 3600.0) as u64) })
        })
    }
}

/// The battery from `pmset -g batt`, as in `-InternalBattery-0 (id=…)  85%; discharging; 3:45 remaining` (a tab after the id).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn parse_pmset(text: &str) -> Option<Battery> {
    let line = text.lines().find(|l| l.contains("InternalBattery"))?;
    let fields: Vec<&str> = line.split(';').map(str::trim).collect();
    let percent: f32 =
        fields.first()?.rsplit(char::is_whitespace).next()?.trim_end_matches('%').parse().ok()?;
    let charge = match fields.get(1).copied() {
        Some("charging") => Charge::Charging,
        Some("discharging") => Charge::Discharging,
        Some("charged") => Charge::Full,
        _ => Charge::Holding,
    };
    let time_left = fields.get(2).and_then(|f| {
        let (h, m) = f.split_whitespace().next()?.split_once(':')?;
        Some(h.parse::<u64>().ok()? * 3600 + m.parse::<u64>().ok()? * 60)
    });
    Some(Battery { percent, charge, time_left })
}

/// Tests for reading the battery.
#[cfg(test)]
mod tests;
