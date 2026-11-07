use super::*;

fn write(name: &str, text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ui-config-test-{}-{name}.toml", std::process::id()));
    std::fs::write(&p, text).unwrap();
    p
}

#[test]
fn the_tool_file_overrides_the_shared_one_and_keys_take_one_or_many() {
    let shared = write(
        "shared",
        "leader = \"ctrl-space\"\n[keys]\n\"nav.down\" = \"n\"\n\"quit\" = [\"q\", \"Q\"]\n",
    );
    let own = write("own", "[keys]\n\"nav.down\" = [\"j\", \"down\"]\n\"help\" = []\n");
    let (cfg, problems) = Config::read(&[shared, own, PathBuf::from("/nonexistent/config.toml")]);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(cfg.leader.as_deref(), Some("ctrl-space"));
    assert_eq!(cfg.keys["nav.down"], ["j", "down"]);
    assert_eq!(cfg.keys["quit"], ["q", "Q"]);
    assert!(cfg.keys["help"].is_empty());
}

#[test]
fn a_broken_file_is_reported_and_skipped() {
    let bad = write("bad", "[keys\n");
    let good = write("good", "leader = \"x\"\n");
    let (cfg, problems) = Config::read(&[bad, good]);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("bad.toml"), "{problems:?}");
    assert_eq!(cfg.leader.as_deref(), Some("x"));
}
