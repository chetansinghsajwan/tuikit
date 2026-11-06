//! Directory hierarchy for the list: folders as header rows, items indented
//! beneath them, folders that only contain one folder merged (`docs/guide/`).
//! Pure data: the picker feeds it the matched candidates and draws the rows.

use std::collections::{BTreeMap, HashMap};

/// One candidate to place: its position in the matched list, the folders above
/// it, and a key that orders it within its folder.
#[derive(Debug, Clone)]
pub struct Entry {
    pub idx: u32,
    pub dirs: Vec<String>,
    pub sort: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// A folder header at `depth`; `label` may span several folders (`a/b`).
    Dir { depth: u16, label: String },
    /// A candidate (its matched-list index) at `depth`.
    Item { idx: u32, depth: u16 },
}

#[derive(Debug, Default)]
pub struct Tree {
    pub rows: Vec<Row>,
    /// Matched-list indices in the order they are drawn (what Up/Down walk).
    pub order: Vec<u32>,
    /// `row_of[idx]`: the row an item is drawn on.
    pub row_of: HashMap<u32, usize>,
    /// `pos_of[idx]`: its position in `order`.
    pub pos_of: HashMap<u32, usize>,
}

#[derive(Default)]
struct Node {
    dirs: BTreeMap<(String, String), Node>,
    items: Vec<(String, u32)>,
    /// The best (lowest) match index anywhere below this folder.
    best: u32,
}

/// `ranked`: order by how well things matched (the folder holding the best match first, the best match first
/// within its folder); otherwise by name, folders before files.
pub fn build(entries: Vec<Entry>, ranked: bool) -> Tree {
    let mut root = Node { best: u32::MAX, ..Node::default() };
    for e in entries {
        let mut node = &mut root;
        node.best = node.best.min(e.idx);
        for d in e.dirs {
            node = node.dirs.entry((d.to_lowercase(), d)).or_insert_with(|| Node { best: u32::MAX, ..Node::default() });
            node.best = node.best.min(e.idx);
        }
        node.items.push((e.sort, e.idx));
    }
    let mut tree = Tree::default();
    emit(&root, 0, ranked, &mut tree);
    tree
}

fn emit(node: &Node, depth: u16, ranked: bool, tree: &mut Tree) {
    // Folders first, then the items of this folder.
    let mut dirs: Vec<(&(String, String), &Node)> = node.dirs.iter().collect();
    if ranked {
        dirs.sort_by_key(|(_, n)| n.best);
    }
    for ((_, name), child) in dirs {
        let mut label = name.clone();
        let mut cur = child;
        while cur.items.is_empty() && cur.dirs.len() == 1 {
            let ((_, next_name), next) = cur.dirs.iter().next().expect("one child");
            label.push('/');
            label.push_str(next_name);
            cur = next;
        }
        tree.rows.push(Row::Dir { depth, label });
        emit(cur, depth + 1, ranked, tree);
    }
    let mut items: Vec<&(String, u32)> = node.items.iter().collect();
    if ranked {
        items.sort_by_key(|(_, idx)| *idx);
    } else {
        items.sort();
    }
    for (_, idx) in items {
        tree.row_of.insert(*idx, tree.rows.len());
        tree.pos_of.insert(*idx, tree.order.len());
        tree.order.push(*idx);
        tree.rows.push(Row::Item { idx: *idx, depth });
    }
}

/// The folder header rows that enclose the row at `at`, outermost first (what stays
/// pinned at the top of the list while that row is the first one shown).
pub fn ancestors(rows: &[Row], at: usize) -> Vec<usize> {
    let depth = match rows.get(at) {
        Some(Row::Dir { depth, .. }) | Some(Row::Item { depth, .. }) => *depth,
        None => return Vec::new(),
    };
    let mut need = depth;
    let mut found = Vec::new();
    for i in (0..at).rev() {
        if need == 0 {
            break;
        }
        if let Row::Dir { depth: d, .. } = &rows[i] {
            if *d < need {
                found.push(i);
                need = *d;
            }
        }
    }
    found.reverse();
    found
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod ranked_tests;
