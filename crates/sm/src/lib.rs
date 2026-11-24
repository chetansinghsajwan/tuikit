//! The system monitor: CPU, memory, disks, network and a process list that sorts, filters, shows a
//! tree and sends signals. `sample` reads the machine, `procs` orders the process list, `format`
//! writes sizes and times, and `monitor` is the screen the shared app shell hosts.

/// Sizes, rates and durations as short text.
pub mod format;
/// The monitor's actions and default keys.
pub mod keys;
/// The screen: panes of graphs and the process list.
pub mod monitor;
/// Which panes are shown and where they go.
pub mod panels;
/// The process list: sorting, filtering and the tree order.
pub mod procs;
/// Readings of the machine and their history.
pub mod sample;
