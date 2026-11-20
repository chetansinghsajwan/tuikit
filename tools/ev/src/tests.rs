use super::*;

/// The text of a rendered line, without styles.
fn text(line: &Line) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn secret_names_are_found_by_part_and_case() {
    assert!(is_secret("GITHUB_TOKEN"));
    assert!(is_secret("api_key"));
    assert!(is_secret("DB_PASSWORD"));
    assert!(!is_secret("HOME"));
    assert!(!is_secret("PATH"));
}

#[test]
fn a_secret_row_hides_its_value_but_the_candidate_keeps_it() {
    let vars = vec![("B_TOKEN".to_string(), "s3cret".to_string()), ("A".to_string(), "1".to_string())];
    let c = candidates(vars.clone().into_iter(), false);
    assert_eq!(c[0].display, "A=1");
    assert_eq!(c[1].display, format!("B_TOKEN={MASK}"));
    assert_eq!(c[1].output(), "B_TOKEN=s3cret");
    let shown = candidates(vars.into_iter(), true);
    assert_eq!(shown[1].display, "B_TOKEN=s3cret");
}

#[test]
fn a_multi_line_value_stays_on_one_row() {
    assert_eq!(row("X", "a\nb", false), "X=a b");
}

#[test]
fn dir_lists_are_found_by_name_or_by_shape() {
    assert!(is_dir_list("PATH", ""));
    assert!(is_dir_list("XDG_DATA_DIRS", "/a"));
    assert!(is_dir_list("FOO", "/a:/b"));
    assert!(!is_dir_list("FOO", "a:b"));
    assert!(!is_dir_list("FOO", "plain"));
}

#[test]
fn a_path_preview_flags_missing_and_duplicate_entries() {
    let theme = Theme::default();
    let lines = preview("PATH=/:/nonexistent-ev-dir:/", 40, &theme, false);
    let rows: Vec<String> = lines.iter().map(text).collect();
    assert_eq!(rows[0], "3 entries");
    assert_eq!(rows[2], " 1  /");
    assert!(rows[3].ends_with("missing"), "{rows:?}");
    assert!(rows[4].ends_with("duplicate"), "{rows:?}");
}

#[test]
fn a_secret_preview_is_hidden_until_revealed() {
    let theme = Theme::default();
    let hidden = preview("API_KEY=abc", 40, &theme, false);
    assert!(!text(&hidden[0]).contains("abc"));
    let shown = preview("API_KEY=abc", 40, &theme, true);
    assert!(shown.iter().any(|l| text(l) == "abc"));
}

#[test]
fn a_long_value_wraps_to_the_pane_width() {
    let theme = Theme::default();
    let lines = preview("X=abcdefgh", 4, &theme, false);
    let rows: Vec<String> = lines.iter().map(text).collect();
    assert_eq!(&rows[2..], ["abcd", "efgh"]);
}
