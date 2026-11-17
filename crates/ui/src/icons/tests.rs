use super::*;

const icons: IconProvider = IconProvider;

#[test]
fn tables_are_sorted() {
    for t in [DIRS, FILES, EXTS] {
        assert!(t.windows(2).all(|w| w[0].0 < w[1].0));
    }
}

#[test]
fn titles_get_icons_whatever_the_case() {
    assert_eq!(icons.title("Commits"), Some(GIT));
    assert_ne!(icons.title("Shell History"), Some(GIT));
    assert_ne!(icons.title("Clipboard History"), Some(GIT));
    assert_ne!(icons.title("Shell History"), icons.title("Clipboard History"));
    assert_eq!(icons.title("history"), icons.title("HISTORY"));
    assert_eq!(icons.title("Files"), Some(DIR_OPEN));
    assert!(
        icons.title("Branches").is_some()
            && icons.title("Buffers").is_some()
            && icons.title("grep").is_some()
    );
    assert_ne!(icons.title("Branches"), icons.title("History"));
    assert_eq!(icons.title("something else"), None);
}

#[test]
fn lookup_order() {
    assert_eq!(
        icons.file("Cargo.toml", false),
        find(FILES, "cargo.toml").unwrap_or_else(|| find(EXTS, "toml").unwrap())
    );
    assert_eq!(icons.file("main.RS", false), find(EXTS, "rs").unwrap());
    assert_eq!(icons.file("noext", false), FILE);
    assert_eq!(icons.file("run", true), EXEC);
    assert_eq!(icons.dir(".git", false), find(DIRS, ".git").unwrap());
    assert_eq!(icons.dir("src", true), DIR_OPEN);
}
