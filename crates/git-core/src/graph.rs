//! The commit graph: for each commit of a topologically ordered list (children before
//! parents), the cells of a lane drawing like `git log --graph`.
//!
//! Every lane is a column of the drawing that waits for one commit. A commit sits in the
//! lane waiting for it (or starts a new one) and hands that lane to its first parent; its
//! other parents get lanes of their own, drawn as a join to the right.

use crate::CommitInfo;

/// One drawn row: the glyph and the lane (colour index) of each cell, two cells per lane.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GraphRow {
    pub cells: Vec<(char, usize)>,
}

impl GraphRow {
    /// The row as text.
    pub fn text(&self) -> String {
        self.cells.iter().map(|c| c.0).collect()
    }
}

/// Lanes of the drawing for `commits` (newest first, children before their parents).
pub fn graph(commits: &[CommitInfo]) -> Vec<GraphRow> {
    let mut lanes: Vec<Option<String>> = Vec::new();
    let mut rows = Vec::with_capacity(commits.len());
    for c in commits {
        let incoming: Vec<usize> = lanes.iter().enumerate().filter(|(_, w)| w.as_deref() == Some(c.oid.as_str())).map(|(i, _)| i).collect();
        let col = match incoming.first() {
            Some(&i) => i,
            None => match lanes.iter().position(Option::is_none) {
                Some(i) => i,
                None => {
                    lanes.push(None);
                    lanes.len() - 1
                }
            },
        };
        // Joins: (lane, glyph) drawn with a horizontal line from the commit's lane.
        let mut joins: Vec<(usize, char)> = Vec::new();
        for &i in incoming.iter().skip(1) {
            joins.push((i, if i > col { '╯' } else { '╰' }));
            lanes[i] = None;
        }
        // The first parent continues in this lane, unless another lane already waits for it.
        let mut parents = c.parents.iter();
        lanes[col] = None;
        if let Some(p) = parents.next() {
            match lanes.iter().position(|w| w.as_deref() == Some(p.as_str())) {
                Some(j) => joins.push((j, if j > col { '┤' } else { '├' })),
                None => lanes[col] = Some(p.clone()),
            }
        }
        for p in parents {
            match lanes.iter().position(|w| w.as_deref() == Some(p.as_str())) {
                Some(j) => joins.push((j, if j > col { '┤' } else { '├' })),
                None => {
                    let j = match lanes.iter().enumerate().position(|(i, w)| w.is_none() && i != col && !incoming.contains(&i)) {
                        Some(j) => j,
                        None => {
                            lanes.push(None);
                            lanes.len() - 1
                        }
                    };
                    lanes[j] = Some(p.clone());
                    joins.push((j, if j > col { '╮' } else { '╭' }));
                }
            }
        }
        // The lanes still alive after this row (a lane that ended here still shows its join).
        let width = lanes.len().max(incoming.iter().max().map_or(0, |i| i + 1)).max(col + 1);
        let mut cells: Vec<(char, usize)> = Vec::with_capacity(width * 2);
        for i in 0..width {
            let glyph = if i == col {
                '●'
            } else if let Some(&(_, g)) = joins.iter().find(|(j, _)| *j == i) {
                g
            } else if lanes.get(i).is_some_and(Option::is_some) || (incoming.contains(&i) && lanes.get(i).is_some_and(Option::is_some)) {
                '│'
            } else {
                ' '
            };
            cells.push((glyph, i));
            cells.push((' ', i));
        }
        // A horizontal line from the commit to each join, crossing the lanes between.
        for &(j, _) in &joins {
            let (lo, hi) = (col.min(j), col.max(j));
            for k in (2 * lo + 1)..(2 * hi) {
                let lane = j;
                if k % 2 == 1 {
                    cells[k] = ('─', lane);
                } else if cells[k].0 == '│' {
                    cells[k] = ('┼', lane);
                } else if cells[k].0 == ' ' {
                    cells[k] = ('─', lane);
                }
            }
        }
        while lanes.last().is_some_and(Option::is_none) {
            lanes.pop();
        }
        while cells.last().is_some_and(|c| c.0 == ' ') {
            cells.pop();
        }
        rows.push(GraphRow { cells });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(oid: &str, parents: &[&str]) -> CommitInfo {
        CommitInfo { oid: oid.into(), short: oid.into(), subject: String::new(), message: String::new(), author: String::new(), email: String::new(), time: 0, parents: parents.iter().map(|p| p.to_string()).collect() }
    }

    fn drawn(commits: &[CommitInfo]) -> Vec<String> {
        graph(commits).iter().map(GraphRow::text).collect()
    }

    #[test]
    fn a_straight_history_is_one_lane() {
        assert_eq!(drawn(&[c("a", &["b"]), c("b", &["c"]), c("c", &[])]), vec!["●", "●", "●"]);
    }

    #[test]
    fn a_merge_forks_a_lane_and_the_branches_join_again() {
        // a merges b and c; both have the parent d.
        let rows = drawn(&[c("a", &["b", "c"]), c("b", &["d"]), c("c", &["d"]), c("d", &[])]);
        assert_eq!(rows, vec!["●─╮", "● │", "├─●", "●"]);
    }

    #[test]
    fn a_branch_tip_without_a_child_starts_its_own_lane() {
        // x is a second tip next to a; both continue to the same root.
        let rows = drawn(&[c("a", &["r"]), c("x", &["r"]), c("r", &[])]);
        assert_eq!(rows, vec!["●", "├─●", "●"]);
    }
}
