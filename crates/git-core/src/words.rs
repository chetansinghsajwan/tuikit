//! Word-level changes inside a changed line (what `delta` calls intra-line highlighting): the two lines
//! are split into tokens (a word, a run of spaces, one symbol), diffed, and the changed tokens come back as
//! byte ranges of each line.

use gix::diff::blob::{Algorithm, Diff, InternedInput};
use std::ops::Range;

/// What differs between two versions of a line.
#[derive(Debug, Clone, PartialEq)]
pub struct Intraline {
    /// Byte ranges of the old line that are gone or changed.
    pub old: Vec<Range<usize>>,
    /// Byte ranges of the new line that are new or changed.
    pub new: Vec<Range<usize>>,
    /// How much of the two lines is shared, 0.0 (nothing) to 1.0 (equal): marking the words of lines that have
    /// little in common only adds noise, so callers can skip those.
    pub similarity: f32,
}

/// The tokens of `s` with their byte offsets: a run of letters, digits and `_`, a run of whitespace, or one other char.
fn tokens(s: &str) -> Vec<(usize, &str)> {
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            0
        } else if c.is_whitespace() {
            1
        } else {
            2
        }
    };
    let mut out = Vec::new();
    let mut chars = s.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let k = class(c);
        let mut end = start + c.len_utf8();
        if k != 2 {
            while let Some(&(i, n)) = chars.peek() {
                if class(n) != k {
                    break;
                }
                end = i + n.len_utf8();
                chars.next();
            }
        }
        out.push((start, &s[start..end]));
    }
    out
}

/// The word-level difference between `old` and `new`.
pub fn intraline(old: &str, new: &str) -> Intraline {
    let (a, b) = (tokens(old), tokens(new));
    let (at, bt): (Vec<&str>, Vec<&str>) = (a.iter().map(|t| t.1).collect(), b.iter().map(|t| t.1).collect());
    let mut input: InternedInput<&str> = InternedInput::default();
    input.update_before(at.iter().copied());
    input.update_after(bt.iter().copied());
    let diff = Diff::compute(Algorithm::Histogram, &input);
    let span = |toks: &[(usize, &str)], r: Range<u32>| -> Option<Range<usize>> {
        let (from, to) = (r.start as usize, r.end as usize);
        if from >= to {
            return None;
        }
        Some(toks[from].0..toks[to - 1].0 + toks[to - 1].1.len())
    };
    let (mut o, mut n) = (Vec::new(), Vec::new());
    let (mut gone, mut fresh) = (0usize, 0usize);
    for h in diff.hunks() {
        gone += (h.before.end - h.before.start) as usize;
        fresh += (h.after.end - h.after.start) as usize;
        o.extend(span(&a, h.before.clone()));
        n.extend(span(&b, h.after.clone()));
    }
    let total = a.len().max(b.len()).max(1);
    let changed = gone.max(fresh);
    Intraline { old: o, new: n, similarity: 1.0 - changed as f32 / total as f32 }
}

#[cfg(test)]
mod tests;
