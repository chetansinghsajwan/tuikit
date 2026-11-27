//! Round values for the top of a graph, so its scale does not creep with every reading.

/// The smallest of `steps` (ascending) that is at least `value`; the last one when `value` is
/// beyond them all.
pub fn step_up(steps: &[f32], value: f32) -> f32 {
    steps.iter().copied().find(|step| *step >= value).or(steps.last().copied()).unwrap_or(value)
}
