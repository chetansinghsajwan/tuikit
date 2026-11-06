use super::*;

fn file(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ui-recent-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    d.join("state/recent")
}

#[test]
fn the_newest_comes_first_repeats_move_up_and_tools_are_separate() {
    let f = file("order");
    assert!(load_from(&f, "files").is_empty());
    record_to(&f, "files", "a");
    record_to(&f, "files", "b");
    record_to(&f, "git", "x");
    record_to(&f, "files", "a");
    assert_eq!(load_from(&f, "files"), vec!["a", "b"]);
    assert_eq!(load_from(&f, "git"), vec!["x"]);
}

#[test]
fn only_the_newest_few_per_tool_are_kept() {
    let f = file("cap");
    record_to(&f, "other", "keep");
    for i in 0..30 {
        record_to(&f, "files", &format!("id{i}"));
    }
    let got = load_from(&f, "files");
    assert_eq!(got.len(), KEPT);
    assert_eq!((got[0].as_str(), got[KEPT - 1].as_str()), ("id29", "id10"));
    assert_eq!(load_from(&f, "other"), vec!["keep"]);
}

#[test]
fn order_puts_recents_first_and_ignores_unknown_ids() {
    let items = vec!["new", "rename", "delete", "paste"];
    let recent = vec!["paste".to_string(), "gone".to_string(), "new".to_string()];
    assert_eq!(order(items, &recent, |s| s), vec!["paste", "new", "rename", "delete"]);
}

#[test]
fn a_missing_or_garbled_file_means_no_history() {
    let f = file("garbled");
    fs::create_dir_all(f.parent().unwrap()).unwrap();
    fs::write(&f, "no tab here\n\tx\nfiles\tok\n").unwrap();
    assert_eq!(load_from(&f, "files"), vec!["ok"]);
}
