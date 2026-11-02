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

/// `commits` in the order of `git log --topo-order`: every commit before its parents, and among the
/// commits ready to go the newest first (ties keep their incoming order). Commit times alone are not
/// enough, a parent can carry the same or a later time than its child.
pub fn topo_order(commits: Vec<CommitInfo>) -> Vec<CommitInfo> {
    use std::cmp::Reverse;
    use std::collections::{BinaryHeap, HashMap};
    let index: HashMap<String, usize> = commits.iter().enumerate().map(|(i, c)| (c.oid.clone(), i)).collect();
    // How many commits of the list still have to come before each one.
    let mut waiting = vec![0usize; commits.len()];
    for c in &commits {
        for p in &c.parents {
            if let Some(&j) = index.get(p) {
                waiting[j] += 1;
            }
        }
    }
    let mut ready: BinaryHeap<(i64, Reverse<usize>)> = (0..commits.len()).filter(|&i| waiting[i] == 0).map(|i| (commits[i].time, Reverse(i))).collect();
    let mut order = Vec::with_capacity(commits.len());
    while let Some((_, Reverse(i))) = ready.pop() {
        order.push(i);
        for p in &commits[i].parents {
            if let Some(&j) = index.get(p) {
                waiting[j] -= 1;
                if waiting[j] == 0 {
                    ready.push((commits[j].time, Reverse(j)));
                }
            }
        }
    }
    let mut slots: Vec<Option<CommitInfo>> = commits.into_iter().map(Some).collect();
    order.into_iter().filter_map(|i| slots[i].take()).collect()
}

/// The commits `keep` accepts, with each parent link moved to the nearest kept ancestor, so a
/// filtered history still draws as one connected graph (like `git log --graph` with a filter).
/// `commits` is newest first, children before parents.
pub fn simplify(commits: Vec<CommitInfo>, keep: impl Fn(&CommitInfo) -> bool) -> Vec<CommitInfo> {
    use std::collections::{HashMap, HashSet};
    let index: HashMap<&str, usize> = commits.iter().enumerate().map(|(i, c)| (c.oid.as_str(), i)).collect();
    let kept: Vec<bool> = commits.iter().map(|c| keep(c)).collect();
    // For every commit, the kept commits reached first going down through its parents.
    // Parents come later in the list, so walking it backwards has them done already.
    let mut below: Vec<Vec<String>> = vec![Vec::new(); commits.len()];
    for i in (0..commits.len()).rev() {
        let mut found: Vec<String> = Vec::new();
        for p in &commits[i].parents {
            let Some(&j) = index.get(p.as_str()) else { continue };
            if kept[j] {
                found.push(p.clone());
            } else {
                found.extend(below[j].iter().cloned());
            }
        }
        let mut seen = HashSet::new();
        found.retain(|o| seen.insert(o.clone()));
        below[i] = found;
    }
    commits
        .into_iter()
        .enumerate()
        .filter(|(i, _)| kept[*i])
        .map(|(i, mut c)| {
            c.parents = std::mem::take(&mut below[i]);
            c
        })
        .collect()
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
    fn simplify_keeps_the_graph_connected_around_dropped_commits() {
        // a - b - c - d, keeping a, c, d: a's parent becomes c.
        let list = vec![c("a", &["b"]), c("b", &["c"]), c("c", &["d"]), c("d", &[])];
        let kept = simplify(list, |k| k.oid != "b");
        let parents: Vec<(String, Vec<String>)> = kept.iter().map(|k| (k.oid.clone(), k.parents.clone())).collect();
        assert_eq!(parents, vec![("a".into(), vec!["c".into()]), ("c".into(), vec!["d".into()]), ("d".into(), vec![])]);
        // A merge whose two sides are both dropped keeps one link per kept ancestor, once.
        let list = vec![c("m", &["x", "y"]), c("x", &["r"]), c("y", &["r"]), c("r", &[])];
        let kept = simplify(list, |k| k.oid == "m" || k.oid == "r");
        assert_eq!(kept[0].parents, vec!["r".to_string()]);
    }

    #[test]
    fn topo_order_puts_children_before_parents_even_with_equal_times() {
        let mut list = vec![c("p", &[]), c("k", &["p"])];
        list[0].time = 5;
        list[1].time = 5;
        let order: Vec<String> = topo_order(list).into_iter().map(|k| k.oid).collect();
        assert_eq!(order, vec!["k".to_string(), "p".to_string()]);
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
