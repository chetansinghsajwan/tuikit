use super::*;

fn e(idx: u32, path: &str) -> Entry {
    let mut parts: Vec<String> = path.split('/').map(String::from).collect();
    let name = parts.pop().unwrap();
    Entry { idx, dirs: parts, sort: name.to_lowercase() }
}

#[test]
fn ancestors_are_the_enclosing_headers_outermost_first() {
    let t = build(
        vec![e(0, "README.md"), e(1, "src/b.rs"), e(2, "src/ui/c.rs"), e(3, "docs/x.md")],
        false,
    );
    // rows: docs/, #3, src/, ui/, #2, #1, #0
    assert_eq!(ancestors(&t.rows, 4), vec![2, 3]); // c.rs: src, ui
    assert_eq!(ancestors(&t.rows, 5), vec![2]); // b.rs: src
    assert_eq!(ancestors(&t.rows, 3), vec![2]); // ui/ itself: src
    assert!(ancestors(&t.rows, 6).is_empty()); // README: top level
    assert!(ancestors(&t.rows, 0).is_empty() && ancestors(&t.rows, 99).is_empty());
}

fn shape(t: &Tree) -> Vec<String> {
    t.rows
        .iter()
        .map(|r| match r {
            Row::Dir { depth, label } => format!("{}{label}/", "  ".repeat(*depth as usize)),
            Row::Head { depth, label, .. } => format!("{}{label}/", "  ".repeat(*depth as usize)),
            Row::Item { idx, depth } => format!("{}#{idx}", "  ".repeat(*depth as usize)),
        })
        .collect()
}

#[test]
fn folders_first_items_indented_and_single_chains_merged() {
    let t = build(
        vec![
            e(0, "README.md"),
            e(1, "src/b.rs"),
            e(2, "src/a.rs"),
            e(3, "src/ui/c.rs"),
            e(4, "docs/guide/x.md"),
            e(5, "Zed/y.txt"),
        ],
        false,
    );
    assert_eq!(
        shape(&t),
        vec![
            "docs/guide/", // docs only holds guide: merged
            "  #4",
            "src/",
            "  ui/",
            "    #3",
            "  #2", // a.rs before b.rs
            "  #1",
            "Zed/", // folders sort case-insensitively
            "  #5",
            "#0",
        ]
    );
    // The walk order skips headers and follows the drawing order.
    assert_eq!(t.order, vec![4, 3, 2, 1, 5, 0]);
    assert_eq!(t.pos_of[&2], 2);
    assert_eq!(t.row_of[&0], 9);
}

#[test]
fn a_folder_with_items_or_siblings_is_not_merged() {
    let t = build(vec![e(0, "a/x.rs"), e(1, "a/b/y.rs"), e(2, "a/c/z.rs")], false);
    assert_eq!(shape(&t), vec!["a/", "  b/", "    #1", "  c/", "    #2", "  #0"]);
}

#[test]
fn empty_and_flat_inputs() {
    assert!(build(vec![], false).rows.is_empty());
    let t = build(vec![e(0, "b.txt"), e(1, "a.txt")], false);
    assert_eq!(shape(&t), vec!["#1", "#0"]);
}

fn hier(idx: u32, id: u32, order: u32, parents: &[(u32, &str)]) -> HierEntry {
    HierEntry {
        idx,
        hier: crate::candidate::Hier {
            id,
            kind: None,
            order,
            ancestors: parents
                .iter()
                .map(|(id, label)| crate::candidate::Ancestor { id: *id, kind: None, label: label.to_string(), order: 0 })
                .collect(),
        },
    }
}

#[test]
fn symbols_nest_under_shared_parents() {
    // `S` is matched itself, `T` is only there as the header of `t`.
    let t = build_hier(
        vec![hier(0, 1, 0, &[]), hier(1, 2, 1, &[(1, "S")]), hier(2, 4, 5, &[(3, "T")])],
        false,
    );
    assert_eq!(shape(&t), ["#0", "  #1", "T/", "  #2"]);
    assert_eq!(t.order, [0, 1, 2]);
}

#[test]
fn a_matched_parent_is_one_row() {
    // The child arrives first and names its parent; the parent's own entry fills that row in.
    let t = build_hier(vec![hier(0, 2, 1, &[(1, "S")]), hier(1, 1, 0, &[])], false);
    assert_eq!(shape(&t), ["#1", "  #0"]);
}
