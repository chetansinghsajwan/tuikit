//! The process list as the monitor shows it: which processes, in which order, and how deep in the
//! tree. Pure data in, rows out, so the screen only draws what this decides.

use std::cmp::Ordering;
use std::collections::HashMap;

/// One process as read from the machine.
#[derive(Debug, Clone, PartialEq)]
pub struct Proc {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    /// Share of one core, in percent (may pass 100 on several cores).
    pub cpu: f32,
    /// Resident memory, in bytes.
    pub memory: u64,
    pub status: String,
}

/// What the list is ordered by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Cpu,
    Memory,
    Pid,
    Name,
}

impl Sort {
    /// The next order in the cycle.
    pub fn next(self) -> Sort {
        match self {
            Sort::Cpu => Sort::Memory,
            Sort::Memory => Sort::Pid,
            Sort::Pid => Sort::Name,
            Sort::Name => Sort::Cpu,
        }
    }

    /// The name shown in the title.
    pub fn label(self) -> &'static str {
        match self {
            Sort::Cpu => "cpu",
            Sort::Memory => "memory",
            Sort::Pid => "pid",
            Sort::Name => "name",
        }
    }

    /// The title of the column this order sorts by.
    pub fn column(self) -> &'static str {
        match self {
            Sort::Cpu => "CPU%",
            Sort::Memory => "MEM",
            Sort::Pid => "PID",
            Sort::Name => "NAME",
        }
    }

    /// Whether the largest comes first by default (load) or the smallest (pid, name).
    fn is_descending(self) -> bool {
        matches!(self, Sort::Cpu | Sort::Memory)
    }
}

/// How the list is shown: the order, whether it is reversed, the filter text and flat or tree.
#[derive(Debug, Clone)]
pub struct Options {
    pub sort: Sort,
    pub is_reversed: bool,
    pub filter: String,
    pub is_tree: bool,
}

impl Options {
    /// Whether the list now runs from the largest to the smallest, for the sort marker.
    pub fn is_descending(&self) -> bool {
        self.sort.is_descending() != self.is_reversed
    }
}

/// A row of the list: the process (an index into the slice given to [`rows`]) and its depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub index: usize,
    pub depth: usize,
}

/// Whether `a` comes before `b` under `sort`, ties broken by pid so the order never jitters.
fn compare(a: &Proc, b: &Proc, sort: Sort, is_reversed: bool) -> Ordering {
    let by = match sort {
        Sort::Cpu => a.cpu.total_cmp(&b.cpu),
        Sort::Memory => a.memory.cmp(&b.memory),
        Sort::Pid => a.pid.cmp(&b.pid),
        Sort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    };
    let by = if sort.is_descending() { by.reverse() } else { by };
    let by = if is_reversed { by.reverse() } else { by };
    by.then(a.pid.cmp(&b.pid))
}

/// The rows to show. A filter (matching the name or the pid) always gives a flat list; otherwise
/// the tree option nests children under their parent, siblings in the chosen order.
pub fn rows(procs: &[Proc], options: &Options) -> Vec<Row> {
    let filter = options.filter.trim().to_lowercase();
    let mut order: Vec<usize> = (0..procs.len())
        .filter(|&i| {
            filter.is_empty()
                || procs[i].name.to_lowercase().contains(&filter)
                || procs[i].pid.to_string() == filter
        })
        .collect();
    order.sort_by(|&a, &b| compare(&procs[a], &procs[b], options.sort, options.is_reversed));
    if !filter.is_empty() || !options.is_tree {
        return order.into_iter().map(|index| Row { index, depth: 0 }).collect();
    }
    tree(procs, &order)
}

/// `order` (already sorted) arranged depth first under each parent; a process whose parent is not
/// in the list is a root.
fn tree(procs: &[Proc], order: &[usize]) -> Vec<Row> {
    let known: HashMap<u32, usize> = order.iter().map(|&i| (procs[i].pid, i)).collect();
    let mut children: HashMap<u32, Vec<usize>> = HashMap::new();
    let mut roots = Vec::new();
    for &i in order {
        match procs[i].parent.filter(|p| known.contains_key(p) && *p != procs[i].pid) {
            Some(parent) => children.entry(parent).or_default().push(i),
            None => roots.push(i),
        }
    }
    let mut out = Vec::with_capacity(order.len());
    let mut stack: Vec<(usize, usize)> = roots.into_iter().rev().map(|i| (i, 0)).collect();
    while let Some((index, depth)) = stack.pop() {
        out.push(Row { index, depth });
        if let Some(kids) = children.get(&procs[index].pid) {
            stack.extend(kids.iter().rev().map(|&k| (k, depth + 1)));
        }
    }
    out
}

/// Tests for the process list.
#[cfg(test)]
mod tests;
