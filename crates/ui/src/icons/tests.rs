use super::*;

#[test]
fn tables_are_sorted() {
    for t in [DIRS, FILES, EXTS] {
        assert!(t.windows(2).all(|w| w[0].0 < w[1].0));
    }
}

#[test]
fn titles_get_icons_whatever_the_case() {
    assert_eq!(for_title("Commits"), Some(GIT));
    assert_ne!(for_title("Shell History"), Some(GIT));
    assert_ne!(for_title("Clipboard History"), Some(GIT));
    assert_ne!(for_title("Shell History"), for_title("Clipboard History"));
    assert_eq!(for_title("history"), for_title("HISTORY"));
    assert_eq!(for_title("Files"), Some(DIR_OPEN));
    assert!(
        for_title("Branches").is_some()
            && for_title("Buffers").is_some()
            && for_title("grep").is_some()
    );
    assert_ne!(for_title("Branches"), for_title("History"));
    assert_eq!(for_title("something else"), None);
}

#[test]
fn lookup_order() {
    assert_eq!(
        file("Cargo.toml", false),
        find(FILES, "cargo.toml").unwrap_or_else(|| find(EXTS, "toml").unwrap())
    );
    assert_eq!(file("main.RS", false), find(EXTS, "rs").unwrap());
    assert_eq!(file("noext", false), FILE);
    assert_eq!(file("run", true), EXEC);
    assert_eq!(dir(".git", false), find(DIRS, ".git").unwrap());
    assert_eq!(dir("src", true), DIR_OPEN);
}
