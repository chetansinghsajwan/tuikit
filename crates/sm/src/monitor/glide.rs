//! The values the bars show, each gliding to the latest reading instead of jumping.

use crate::sample::{Snapshot, ratio};
use std::time::Instant;
use ui::graph::Eased;

/// The bars that glide: the cores, memory, swap and every disk.
#[derive(Default)]
pub(super) struct Glide {
    pub cores: Vec<Eased>,
    pub memory: Option<Eased>,
    pub swap: Option<Eased>,
    pub disks: Vec<Eased>,
}

impl Glide {
    /// Sends every bar toward the readings of `s`.
    pub fn retarget(&mut self, s: &Snapshot, now: Instant) {
        let cores: Vec<f32> = s.cores.iter().map(|c| c / 100.0).collect();
        let disks: Vec<f32> = s.disks.iter().map(|d| ratio(d.used, d.total)).collect();
        Self::follow(&mut self.cores, &cores, now);
        Self::follow(&mut self.disks, &disks, now);
        let memory = ratio(s.memory_used, s.memory_total);
        self.memory.get_or_insert(Eased::new(memory)).set(memory, now);
        let swap = ratio(s.swap_used, s.swap_total);
        self.swap.get_or_insert(Eased::new(swap)).set(swap, now);
    }

    /// Makes `bars` as many as `targets` (new ones start at their target) and sets each target.
    fn follow(bars: &mut Vec<Eased>, targets: &[f32], now: Instant) {
        bars.truncate(targets.len());
        for (i, target) in targets.iter().enumerate() {
            match bars.get_mut(i) {
                Some(bar) => bar.set(*target, now),
                None => bars.push(Eased::new(*target)),
            }
        }
    }

    /// Whether any bar is still on its way.
    pub fn is_moving(&self, now: Instant) -> bool {
        self.cores
            .iter()
            .chain(&self.disks)
            .chain(self.memory.iter())
            .chain(self.swap.iter())
            .any(|bar| bar.is_moving(now))
    }
}

/// The value of bar `i` of `bars` at `now`, or `fallback` when there is no such bar.
pub(super) fn shown(bars: &[Eased], i: usize, now: Instant, fallback: f32) -> f32 {
    bars.get(i).map_or(fallback, |bar| bar.value(now))
}
