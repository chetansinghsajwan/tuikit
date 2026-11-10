use super::*;

const SAMPLE: &str = r#"
(defsrc
  q     w     e
  a     s     spc
)
(platform (macos)
  (defalias cut M-x)
)
(platform (win linux)
  (defalias cut C-x)
)
;; a comment with (parens
(deflayermap main
  q @cut
  w (layer-switch ins)
  e (layer-while-held nav)
  a lctl
  s S-1
  ___ XX
)
(deflayermap nav
  q pgup
  ___ _
)
(deflayermap ins
  ___ _
)
"#;

fn keys(label: &str, outputs: &[&str]) -> Action {
    Action::Keys { label: label.into(), outputs: outputs.iter().map(|s| s.to_string()).collect() }
}

#[test]
fn rows_follow_source_lines() {
    let cfg = Config::parse(SAMPLE).unwrap();
    assert_eq!(cfg.rows, vec![vec!["q", "w", "e"], vec!["a", "s", "spc"]]);
}

#[test]
fn actions_of_main() {
    let cfg = Config::parse(SAMPLE).unwrap();
    assert_eq!(cfg.action("main", "main", "q"), keys("cut", &["lctl", "x"]));
    assert_eq!(cfg.action("main", "main", "w"), Action::Switch("ins".into()));
    assert_eq!(cfg.action("main", "main", "e"), Action::Hold("nav".into()));
    assert_eq!(cfg.action("main", "main", "a"), keys("ctl", &["lctl"]));
    assert_eq!(cfg.action("main", "main", "s"), keys("!", &["lsft", "1"]));
    assert_eq!(cfg.action("main", "main", "spc"), Action::Blocked);
}

#[test]
fn transparent_falls_to_the_base_layer_or_the_key_itself() {
    let cfg = Config::parse(SAMPLE).unwrap();
    assert!(cfg.is_overlay("nav"));
    assert!(!cfg.is_overlay("ins"));
    assert_eq!(cfg.action("nav", "main", "q"), keys("pgup", &["pgup"]));
    assert_eq!(cfg.action("nav", "main", "a"), keys("ctl", &["lctl"]));
    assert_eq!(cfg.action("ins", "ins", "a"), keys("a", &["a"]));
}

#[test]
fn missing_pieces_are_errors() {
    assert!(Config::parse("(defcfg)").is_err());
    assert!(Config::parse("(defsrc a)").is_err());
}
