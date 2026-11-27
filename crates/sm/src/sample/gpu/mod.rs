//! The graphics card: how busy it is, its memory and its temperature. An AMD card is read from sysfs,
//! an NVIDIA one by asking `nvidia-smi`; both in the background (asking a tool costs a process), so
//! the screen never waits for it. A machine with neither has none, and the sensors pane says so.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How often the card is read.
const EVERY: Duration = Duration::from_secs(2);

/// The graphics card at one moment.
#[derive(Debug, Clone, PartialEq)]
pub struct Gpu {
    pub name: String,
    /// How busy, 0 to 100.
    pub percent: f32,
    /// Video memory in bytes.
    pub memory_used: u64,
    pub memory_total: u64,
    /// Degrees Celsius, when it says.
    pub celsius: Option<f32>,
}

/// Where the readings come from.
#[derive(Debug, Clone)]
enum Source {
    /// The sysfs folder of an AMD card (`/sys/class/drm/card0/device`).
    Amd(PathBuf),
    /// The path of `nvidia-smi`.
    Nvidia(PathBuf),
}

/// The card, kept up to date by a background thread.
#[derive(Debug, Clone, Default)]
pub struct Watch {
    found: Arc<Mutex<Option<Gpu>>>,
}

impl Watch {
    /// Starts reading the card, if the machine has one that can be read.
    pub fn spawn() -> Watch {
        let watch = Watch::default();
        let Some(source) = find() else { return watch };
        let found = watch.found.clone();
        std::thread::spawn(move || {
            loop {
                let gpu = read(&source);
                match found.lock() {
                    Ok(mut slot) => *slot = gpu,
                    Err(_) => return,
                }
                std::thread::sleep(EVERY);
            }
        });
        watch
    }

    /// The card at its last reading; `None` without one or before the first reading.
    pub fn get(&self) -> Option<Gpu> {
        self.found.lock().ok()?.clone()
    }
}

/// The first card that can be read.
fn find() -> Option<Source> {
    if let Some(dir) = amd_cards(Path::new("/sys/class/drm")).into_iter().next() {
        return Some(Source::Amd(dir));
    }
    // On WSL the tool lives outside the PATH.
    ["nvidia-smi", "/usr/lib/wsl/lib/nvidia-smi"].into_iter().find_map(|tool| {
        let ok = std::process::Command::new(tool).arg("-L").output().ok()?.status.success();
        ok.then(|| PathBuf::from(tool)).map(Source::Nvidia)
    })
}

/// The sysfs folders of the AMD cards under `drm` (those that say how busy they are).
fn amd_cards(drm: &Path) -> Vec<PathBuf> {
    let mut cards: Vec<PathBuf> = std::fs::read_dir(drm)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path().join("device"))
        .filter(|dir| dir.join("gpu_busy_percent").exists())
        .collect();
    cards.sort();
    cards
}

/// The card now.
fn read(source: &Source) -> Option<Gpu> {
    match source {
        Source::Amd(dir) => read_amd(dir),
        Source::Nvidia(tool) => {
            let args = [
                "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu",
                "--format=csv,noheader,nounits",
            ];
            let out = std::process::Command::new(tool).args(args).output().ok()?;
            parse_nvidia(&String::from_utf8_lossy(&out.stdout))
        }
    }
}

/// An AMD card from its sysfs folder `dir`.
pub fn read_amd(dir: &Path) -> Option<Gpu> {
    let field = |name: &str| std::fs::read_to_string(dir.join(name)).ok();
    let number = |name: &str| field(name)?.trim().parse::<u64>().ok();
    let celsius =
        std::fs::read_dir(dir.join("hwmon")).into_iter().flatten().flatten().find_map(|hwmon| {
            let milli = std::fs::read_to_string(hwmon.path().join("temp1_input")).ok()?;
            Some(milli.trim().parse::<f32>().ok()? / 1000.0)
        });
    Some(Gpu {
        name: field("product_name").map_or("amdgpu".into(), |n| n.trim().to_string()),
        percent: number("gpu_busy_percent")? as f32,
        memory_used: number("mem_info_vram_used").unwrap_or(0),
        memory_total: number("mem_info_vram_total").unwrap_or(0),
        celsius,
    })
}

/// The first card in `nvidia-smi --format=csv,noheader,nounits` output: `name, percent, MiB used,
/// MiB total, °C`.
pub fn parse_nvidia(text: &str) -> Option<Gpu> {
    let line = text.lines().next()?;
    let fields: Vec<&str> = line.split(',').map(str::trim).collect();
    let mib = |field: &str| field.parse::<u64>().ok().map(|m| m * 1024 * 1024);
    Some(Gpu {
        name: fields.first()?.to_string(),
        percent: fields.get(1)?.parse().ok()?,
        memory_used: mib(fields.get(2)?).unwrap_or(0),
        memory_total: mib(fields.get(3)?).unwrap_or(0),
        celsius: fields.get(4).and_then(|t| t.parse().ok()),
    })
}

/// Tests for reading the graphics card.
#[cfg(test)]
mod tests;
