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
    /// A header for something that is not a candidate itself (a symbol's container, a file); `kind` is `None` for a file.
    Head { depth: u16, label: String, kind: Option<ui::symbol::Kind> },
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
            node = node
                .dirs
                .entry((d.to_lowercase(), d))
                .or_insert_with(|| Node { best: u32::MAX, ..Node::default() });
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


/// One candidate for the hierarchy built by `build_hier`.
#[derive(Debug, Clone)]
pub struct HierEntry {
    pub idx: u32,
    pub hier: crate::candidate::Hier,
}

struct HNode {
    label: String,
    kind: Option<ui::symbol::Kind>,
    idx: Option<u32>,
    order: u32,
    best: u32,
    children: Vec<usize>,
}

/// Things inside things: every candidate under the ancestors it names, which are shared. An ancestor that
/// is a candidate itself is an item row (it can be picked); one that is not is a header row. `ranked`
/// orders siblings by match quality instead of position.
pub fn build_hier(entries: Vec<HierEntry>, ranked: bool) -> Tree {
    let mut nodes: Vec<HNode> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();
    let mut by_id: HashMap<u32, usize> = HashMap::new();
    for e in entries {
        let mut parent: Option<usize> = None;
        let chain = e
            .hier
            .ancestors
            .iter()
            .map(|a| (a.id, a.kind, a.label.as_str(), a.order, None))
            .chain([(e.hier.id, e.hier.kind, "", e.hier.order, Some(e.idx))]);
        for (id, kind, label, order, idx) in chain {
            let at = *by_id.entry(id).or_insert_with(|| {
                nodes.push(HNode { label: label.to_string(), kind, idx: None, order, best: u32::MAX, children: Vec::new() });
                let at = nodes.len() - 1;
                match parent {
                    Some(p) => nodes[p].children.push(at),
                    None => roots.push(at),
                }
                at
            });
            nodes[at].best = nodes[at].best.min(e.idx);
            if idx.is_some() {
                nodes[at].idx = idx;
                nodes[at].order = order;
            }
            parent = Some(at);
        }
    }
    let mut tree = Tree::default();
    let key = |nodes: &[HNode], i: usize| if ranked { nodes[i].best } else { nodes[i].order };
    fn emit(nodes: &[HNode], at: &[usize], depth: u16, tree: &mut Tree, key: &dyn Fn(&[HNode], usize) -> u32) {
        let mut at = at.to_vec();
        at.sort_by_key(|&i| key(nodes, i));
        for i in at {
            let n = &nodes[i];
            match n.idx {
                Some(idx) => {
                    tree.row_of.insert(idx, tree.rows.len());
                    tree.pos_of.insert(idx, tree.order.len());
                    tree.order.push(idx);
                    tree.rows.push(Row::Item { idx, depth });
                }
                None => tree.rows.push(Row::Head { depth, label: n.label.clone(), kind: n.kind }),
            }
            emit(nodes, &n.children, depth + 1, tree, key);
        }
    }
    emit(&nodes, &roots, 0, &mut tree, &key);
    tree
}
fn row_depth(row: &Row) -> u16 {
    match row {
        Row::Dir { depth, .. } | Row::Head { depth, .. } | Row::Item { depth, .. } => *depth,
    }
}

/// The header rows (and items with others inside them) that enclose the row at `at`, outermost first (what stays
/// pinned at the top of the list while that row is the first one shown).
pub fn ancestors(rows: &[Row], at: usize) -> Vec<usize> {
    let Some(row) = rows.get(at) else { return Vec::new() };
    let depth = row_depth(row);
    let mut need = depth;
    let mut found = Vec::new();
    for i in (0..at).rev() {
        if need == 0 {
            break;
        }
        let d = &row_depth(&rows[i]);
        if *d < need
        {
            found.push(i);
            need = *d;
        }
    }
    found.reverse();
    found
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod ranked_tests;
