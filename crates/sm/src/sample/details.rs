//! Everything the machine says about one process, read when the user asks for it (the list keeps
//! only what fits a row).

use super::{Sampler, user_name};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, UpdateKind};

/// One process in full.
#[derive(Debug, Clone, PartialEq)]
pub struct Details {
    pub pid: u32,
    pub name: String,
    pub parent: Option<(u32, String)>,
    pub user: String,
    pub status: String,
    /// The command line, as one line.
    pub command: String,
    pub exe: Option<String>,
    pub cwd: Option<String>,
    /// Seconds since it started.
    pub run_time: u64,
    pub cpu: f32,
    /// Resident and virtual memory, in bytes.
    pub memory: u64,
    pub virtual_memory: u64,
    pub threads: Option<usize>,
    /// Bytes it has read from and written to disk since it started.
    pub read_total: u64,
    pub written_total: u64,
    /// The processes it started, as pid and name, by pid.
    pub children: Vec<(u32, String)>,
}

impl Sampler {
    /// Reads the parts of process `pid` that the list does not keep up to date (its command line and
    /// working folder), once, so [`Sampler::details`] has them.
    pub fn load_details(&mut self, pid: u32) {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
            false,
            ProcessRefreshKind::nothing()
                .with_cmd(UpdateKind::OnlyIfNotSet)
                .with_cwd(UpdateKind::OnlyIfNotSet)
                .with_exe(UpdateKind::OnlyIfNotSet),
        );
    }

    /// Process `pid` as the last refresh saw it; `None` once it is gone.
    pub fn details(&self, pid: u32) -> Option<Details> {
        let p = self.system.process(Pid::from_u32(pid))?;
        let name_of =
            |pid: Pid| self.system.process(pid).map(|p| p.name().to_string_lossy().into_owned());
        let path = |p: &std::path::Path| p.to_string_lossy().into_owned();
        let usage = p.disk_usage();
        let mut children: Vec<(u32, String)> = self
            .system
            .processes()
            .values()
            .filter(|c| c.parent() == Some(p.pid()) && c.thread_kind().is_none())
            .map(|c| (c.pid().as_u32(), c.name().to_string_lossy().into_owned()))
            .collect();
        children.sort();
        let command: Vec<String> =
            p.cmd().iter().map(|a| a.to_string_lossy().into_owned()).collect();
        Some(Details {
            pid,
            name: p.name().to_string_lossy().into_owned(),
            parent: p.parent().and_then(|parent| Some((parent.as_u32(), name_of(parent)?))),
            user: p.user_id().map_or_else(|| "-".to_string(), |uid| user_name(&self.users, uid)),
            status: p.status().to_string(),
            command: command.join(" "),
            exe: p.exe().map(path),
            cwd: p.cwd().map(path),
            run_time: p.run_time(),
            cpu: p.cpu_usage(),
            memory: p.memory(),
            virtual_memory: p.virtual_memory(),
            threads: p.tasks().map(|t| t.len()),
            read_total: usage.total_read_bytes,
            written_total: usage.total_written_bytes,
            children,
        })
    }
}
