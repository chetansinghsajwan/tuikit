//! Readings of the machine: CPU, memory, disks, network and processes, refreshed on demand, and the
//! history of the few values the monitor graphs. The only module that talks to `sysinfo`.

use crate::procs::Proc;
use std::collections::VecDeque;
use std::time::Instant;
use sysinfo::{Disks, Networks, Pid, ProcessesToUpdate, Signal, System};

/// How many readings of each graphed value are kept.
const HISTORY_SIZE: usize = 300;

/// The last readings of one value, oldest first.
#[derive(Debug, Clone, Default)]
pub struct Ring {
    values: VecDeque<f32>,
}

impl Ring {
    /// Adds a reading, dropping the oldest when the ring is full.
    pub fn push(&mut self, value: f32) {
        if self.values.len() == HISTORY_SIZE {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    /// The readings as 0 to 1 for a graph, scaled by the largest reading but never by less than
    /// `floor` (so a quiet network does not draw its noise at full height).
    pub fn scaled(&self, floor: f32) -> Vec<f32> {
        let max = self.values.iter().copied().fold(floor, f32::max);
        self.values.iter().map(|v| v / max).collect()
    }

    /// The readings as they are.
    pub fn values(&self) -> Vec<f32> {
        self.values.iter().copied().collect()
    }
}

/// One mounted disk.
#[derive(Debug, Clone, PartialEq)]
pub struct DiskInfo {
    pub mount: String,
    pub total: u64,
    pub used: u64,
}

/// The machine at one moment.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub cpu_brand: String,
    /// Total CPU use, 0 to 100.
    pub cpu: f32,
    /// Use of each core, 0 to 100.
    pub cores: Vec<f32>,
    pub load: (f64, f64, f64),
    pub uptime: u64,
    pub memory_used: u64,
    pub memory_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub disks: Vec<DiskInfo>,
    /// Bytes per second received and sent over all interfaces.
    pub download: u64,
    pub upload: u64,
    pub procs: Vec<Proc>,
}

/// Reads the machine and keeps the history of what the monitor graphs.
pub struct Sampler {
    system: System,
    disks: Disks,
    networks: Networks,
    last: Instant,
    pub snapshot: Snapshot,
    pub memory_history: Ring,
    pub download_history: Ring,
    pub upload_history: Ring,
}

impl Sampler {
    /// Opens the readers and takes the first reading. Fails with the reason when the platform
    /// cannot be read.
    pub fn new() -> Result<Sampler, String> {
        let system = System::new_all().map_err(|e| e.to_string())?;
        let disks = Disks::new_with_refreshed_list().map_err(|e| e.to_string())?;
        let networks = Networks::new_with_refreshed_list().map_err(|e| e.to_string())?;
        let mut sampler = Sampler {
            system,
            disks,
            networks,
            last: Instant::now(),
            snapshot: Snapshot::default(),
            memory_history: Ring::default(),
            download_history: Ring::default(),
            upload_history: Ring::default(),
        };
        sampler.refresh();
        Ok(sampler)
    }

    /// Reads everything again and adds to the history.
    pub fn refresh(&mut self) {
        let elapsed = self.last.elapsed().as_secs_f64().max(0.001);
        self.last = Instant::now();
        self.system.refresh_cpu_all();
        self.system.refresh_memory();
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        self.disks.refresh(true);
        self.networks.refresh(true);

        let (received, sent) = self
            .networks
            .values()
            .fold((0, 0), |(r, s), n| (r + n.received(), s + n.transmitted()));
        let s = &mut self.snapshot;
        s.cpu_brand = self.system.cpus().first().map(|c| c.brand().to_string()).unwrap_or_default();
        s.cpu = self.system.global_cpu_usage();
        s.cores = self.system.cpus().iter().map(|c| c.usage()).collect();
        s.load = System::load_average().map(|l| (l.one, l.five, l.fifteen)).unwrap_or_default();
        s.uptime = System::uptime().unwrap_or_default();
        s.memory_used = self.system.used_memory();
        s.memory_total = self.system.total_memory();
        s.swap_used = self.system.used_swap();
        s.swap_total = self.system.total_swap();
        s.download = (received as f64 / elapsed) as u64;
        s.upload = (sent as f64 / elapsed) as u64;
        s.disks = self
            .disks
            .iter()
            .filter(|d| d.total_space() > 0)
            .map(|d| DiskInfo {
                mount: d.mount_point().to_string_lossy().into_owned(),
                total: d.total_space(),
                used: d.total_space() - d.available_space(),
            })
            .collect();
        s.procs = self
            .system
            .processes()
            .values()
            // Linux lists every thread as a process; the monitor shows processes only.
            .filter(|p| p.thread_kind().is_none())
            .map(|p| Proc {
                pid: p.pid().as_u32(),
                parent: p.parent().map(|p| p.as_u32()),
                name: p.name().to_string_lossy().into_owned(),
                cpu: p.cpu_usage(),
                memory: p.memory(),
                status: p.status().to_string(),
            })
            .collect();

        self.memory_history.push(ratio(s.memory_used, s.memory_total));
        self.download_history.push(s.download as f32);
        self.upload_history.push(s.upload as f32);
    }

    /// Sends `signal` to the process `pid`. Returns whether the process exists and took it.
    pub fn signal(&self, pid: u32, signal: Signal) -> bool {
        self.system.process(Pid::from_u32(pid)).and_then(|p| p.kill_with(signal)).unwrap_or(false)
    }
}

/// `used` over `total` as 0 to 1; 0 when there is no total (no swap).
pub fn ratio(used: u64, total: u64) -> f32 {
    if total == 0 { 0.0 } else { (used as f64 / total as f64) as f32 }
}

/// Tests for the history and the ratio.
#[cfg(test)]
mod tests;
