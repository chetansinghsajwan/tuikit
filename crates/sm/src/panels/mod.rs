//! Which panes the monitor shows and where they go. Pure data in, rectangles out, so the screen
//! only draws what this decides.

use ratatui::layout::Rect;

/// One pane of the monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Cpu,
    Memory,
    Network,
    Processes,
}

impl Panel {
    /// The panel named `name` (as the `--hide` flag spells it); `None` if unknown.
    pub fn parse(name: &str) -> Option<Panel> {
        match name.trim().to_ascii_lowercase().as_str() {
            "cpu" => Some(Panel::Cpu),
            "memory" | "mem" => Some(Panel::Memory),
            "network" | "net" => Some(Panel::Network),
            "processes" | "procs" => Some(Panel::Processes),
            _ => None,
        }
    }

    /// The name shown in messages.
    pub fn label(self) -> &'static str {
        match self {
            Panel::Cpu => "cpu",
            Panel::Memory => "memory",
            Panel::Network => "network",
            Panel::Processes => "processes",
        }
    }
}

/// Which panels are on. At least one always is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panels {
    cpu: bool,
    memory: bool,
    network: bool,
    processes: bool,
}

/// Where each shown panel goes; `None` for a hidden one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Placed {
    pub cpu: Option<Rect>,
    pub memory: Option<Rect>,
    pub network: Option<Rect>,
    pub processes: Option<Rect>,
}

impl Default for Panels {
    /// Every panel on.
    fn default() -> Panels {
        Panels { cpu: true, memory: true, network: true, processes: true }
    }
}

impl Panels {
    /// Every panel on except `hidden`; when that would leave none, every panel stays on.
    pub fn without(hidden: &[Panel]) -> Panels {
        let mut panels = Panels::default();
        for panel in hidden {
            panels.toggle(*panel);
        }
        panels
    }

    /// Whether `panel` is on.
    pub fn is_shown(&self, panel: Panel) -> bool {
        *self.flag(panel)
    }

    /// Turns `panel` on or off. Returns `false` and changes nothing when that would hide the last
    /// panel.
    pub fn toggle(&mut self, panel: Panel) -> bool {
        let count =
            [self.cpu, self.memory, self.network, self.processes].iter().filter(|on| **on).count();
        let flag = self.flag_mut(panel);
        if *flag && count == 1 {
            return false;
        }
        *flag = !*flag;
        true
    }

    fn flag(&self, panel: Panel) -> &bool {
        match panel {
            Panel::Cpu => &self.cpu,
            Panel::Memory => &self.memory,
            Panel::Network => &self.network,
            Panel::Processes => &self.processes,
        }
    }

    fn flag_mut(&mut self, panel: Panel) -> &mut bool {
        match panel {
            Panel::Cpu => &mut self.cpu,
            Panel::Memory => &mut self.memory,
            Panel::Network => &mut self.network,
            Panel::Processes => &mut self.processes,
        }
    }

    /// Places the shown panels in `body`: cpu left and memory over network at the right on top, the
    /// processes below. A hidden panel's space goes to the others.
    pub fn place(&self, body: Rect) -> Placed {
        let top_is_on = self.cpu || self.memory || self.network;
        let (top, bottom) = match (top_is_on, self.processes) {
            (true, true) => {
                let height = (body.height * 2 / 5).clamp(6, 18).min(body.height.saturating_sub(3));
                let top = Rect { height, ..body };
                (Some(top), Some(Rect { y: top.bottom(), height: body.height - height, ..body }))
            }
            (true, false) => (Some(body), None),
            (false, _) => (None, Some(body)),
        };
        let mut placed = Placed { processes: bottom, ..Placed::default() };
        let Some(top) = top else { return placed };

        let side_is_on = self.memory || self.network;
        let side = if self.cpu && side_is_on {
            let left_width = top.width * 11 / 20;
            placed.cpu = Some(Rect { width: left_width, ..top });
            Rect { x: top.x + left_width, width: top.width - left_width, ..top }
        } else {
            if self.cpu {
                placed.cpu = Some(top);
            }
            top
        };
        match (self.memory, self.network) {
            (true, true) => {
                let height = side.height * 3 / 5;
                placed.memory = Some(Rect { height, ..side });
                placed.network =
                    Some(Rect { y: side.y + height, height: side.height - height, ..side });
            }
            (true, false) => placed.memory = Some(side),
            (false, true) => placed.network = Some(side),
            (false, false) => {}
        }
        placed
    }
}

/// Tests for the panels.
#[cfg(test)]
mod tests;
