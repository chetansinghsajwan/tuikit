use super::*;

#[test]
fn globs_include_and_exclude_by_comma_list() {
    let d = std::env::temp_dir().join(format!("pick-globs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("src")).unwrap();
    std::fs::create_dir_all(d.join("tests")).unwrap();
    for f in ["src/a.rs", "src/b.txt", "tests/c.rs"] {
        std::fs::write(d.join(f), "x").unwrap();
    }
    let found = |inc: &str, exc: &str| -> Vec<String> {
        let o = Options { include: inc.into(), exclude: exc.into(), ..Options::default() };
        let mut walk = ignore::WalkBuilder::new(&d);
        if let Some(ov) = globs(&d, &o) {
            walk.overrides(ov);
        }
        let mut v: Vec<String> = walk
            .build()
            .flatten()
            .filter(|e| e.path().is_file())
            .map(|e| e.path().strip_prefix(&d).unwrap().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    };
    assert_eq!(found("", "").len(), 3);
    assert_eq!(found("*.rs", ""), ["src/a.rs", "tests/c.rs"]);
    assert_eq!(found("*.rs", "tests/"), ["src/a.rs"]);
    assert_eq!(found("", "*.txt, tests/"), ["src/a.rs"]);
    let _ = std::fs::remove_dir_all(&d);
}
