//! The monitor's actions and the names of its leader menus.

use ui::keys::{Action, Group};

/// Every action of the monitor. One table feeds the keymap, the palette and the help screen, so the
/// three never disagree.
pub const ACTIONS: &[Action] = &[
    Action {
        id: "sort.next",
        name: "next sort",
        desc: "order the processes by the next column (cpu, memory, pid, name)",
        basic: false,
    },
    Action {
        id: "sort.reverse",
        name: "reverse sort",
        desc: "flip the order of the processes",
        basic: false,
    },
    Action {
        id: "view.tree",
        name: "toggle tree",
        desc: "show processes nested under their parent",
        basic: false,
    },
    Action {
        id: "filter.start",
        name: "filter",
        desc: "show only processes whose name or pid matches the text",
        basic: false,
    },
    Action {
        id: "filter.clear",
        name: "clear filter",
        desc: "show every process again",
        basic: false,
    },
    Action {
        id: "proc.term",
        name: "terminate",
        desc: "ask the process to stop (SIGTERM; asks first)",
        basic: false,
    },
    Action {
        id: "proc.kill",
        name: "kill",
        desc: "stop the process at once (SIGKILL; asks first)",
        basic: false,
    },
    Action { id: "panel.cpu", name: "toggle cpu", desc: "show or hide the cpu pane", basic: false },
    Action {
        id: "panel.memory",
        name: "toggle memory",
        desc: "show or hide the memory and disks pane",
        basic: false,
    },
    Action {
        id: "panel.network",
        name: "toggle network",
        desc: "show or hide the network pane",
        basic: false,
    },
    Action {
        id: "panel.processes",
        name: "toggle processes",
        desc: "show or hide the process list",
        basic: false,
    },
    Action { id: "quit", name: "quit", desc: "close the monitor", basic: true },
];

/// Names for the key prefixes, shown when a prefix is pending.
pub const GROUPS: &[Group] = &[
    Group { prefix: "leader p", name: "process" },
    Group { prefix: "leader s", name: "sort" },
    Group { prefix: "leader w", name: "panels" },
    Group { prefix: "leader v", name: "view" },
];
