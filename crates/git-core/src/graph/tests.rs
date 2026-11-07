use super::*;

fn c(oid: &str, parents: &[&str]) -> CommitInfo {
    CommitInfo {
        oid: oid.into(),
        short: oid.into(),
        subject: String::new(),
        message: String::new(),
        author: String::new(),
        email: String::new(),
        time: 0,
        parents: parents.iter().map(|p| p.to_string()).collect(),
    }
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
