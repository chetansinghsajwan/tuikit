use super::*;

#[test]
fn finds_the_category_prefix() {
    assert_eq!(category_len("update: some message"), Some(7));
    assert_eq!(category_len("feat(ui)!: big"), Some(10));
    assert_eq!(category_len("fix(a): b"), Some(7));
    assert_eq!(category_len("not a category: no"), None);
    assert_eq!(category_len("no colon"), None);
    assert_eq!(category_len(": empty"), None);
    assert_eq!(category_len("a/b: path"), None);
    assert_eq!(category_len("fix(: odd"), None);
    assert_eq!(category_len("git panel: one tree"), Some(10));
    assert_eq!(category_len("git panel ui: three words"), None);
    assert_eq!(category_word("Git Panel(x): y").as_deref(), Some("git panel"));
    assert_eq!(category_len("fix  bug: double space"), None);
}

#[test]
fn colors_by_kind_and_splits_the_spans() {
    let t = Theme::default();
    assert_eq!(category_color(&t, "fix: x"), category_color(&t, "fix(ui)!: y"));
    assert_eq!(category_color(&t, "Fix: x"), category_color(&t, "fix: x"));
    assert_eq!(category_word("Feat(ui)!: a").as_deref(), Some("feat"));
    assert_eq!(category_word("plain"), None);
    let s = spans(&t, "docs: write", Style::new());
    assert_eq!((s[0].content.as_ref(), s[1].content.as_ref()), ("docs:", " write"));
    assert_eq!(spans(&t, "plain subject", Style::new()).len(), 1);
}
