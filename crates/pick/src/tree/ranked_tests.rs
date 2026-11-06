use super::*;

#[test]
fn ranked_puts_the_folder_with_the_best_match_first_and_the_best_match_first_inside_it() {
    let e = |idx, dirs: &[&str], name: &str| Entry { idx, dirs: dirs.iter().map(|s| s.to_string()).collect(), sort: name.into() };
    let entries = vec![e(0, &["zed"], "best.rs"), e(1, &["alpha"], "a.rs"), e(2, &["zed"], "a.rs"), e(3, &[], "top.rs")];
    let by_name = build(entries.clone(), false);
    assert_eq!(by_name.order, vec![1, 2, 0, 3]);
    let ranked = build(entries, true);
    assert_eq!(ranked.order, vec![0, 2, 1, 3], "zed (holds #0) first, #0 before #2, then alpha");
}
