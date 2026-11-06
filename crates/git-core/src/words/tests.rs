use super::*;

fn cut<'a>(s: &'a str, ranges: &[Range<usize>]) -> Vec<&'a str> {
    ranges.iter().map(|r| &s[r.clone()]).collect()
}

#[test]
fn a_changed_word_is_the_only_thing_marked() {
    let (old, new) = ("let total = count + 1;", "let total = count + step;");
    let d = intraline(old, new);
    assert_eq!(cut(old, &d.old), vec!["1"]);
    assert_eq!(cut(new, &d.new), vec!["step"]);
    assert!(d.similarity > 0.7, "{}", d.similarity);
}

#[test]
fn insertions_and_deletions_mark_one_side_only() {
    let d = intraline("call(a)", "call(a, b)");
    assert!(d.old.is_empty());
    assert_eq!(cut("call(a, b)", &d.new), vec![", b"]);
    let d = intraline("call(a, b)", "call(a)");
    assert_eq!(cut("call(a, b)", &d.old), vec![", b"]);
    assert!(d.new.is_empty());
}

#[test]
fn unrelated_lines_have_low_similarity_and_equal_lines_none_marked() {
    assert!(intraline("alpha beta gamma", "one two three four").similarity < 0.3);
    let same = intraline("same line", "same line");
    assert!(same.old.is_empty() && same.new.is_empty() && same.similarity > 0.99);
}

#[test]
fn non_ascii_text_stays_on_char_boundaries() {
    let (old, new) = ("héllo wörld", "héllo wörlds");
    let d = intraline(old, new);
    assert!(!d.new.is_empty());
    let _ = cut(new, &d.new); // would panic off a boundary
}
