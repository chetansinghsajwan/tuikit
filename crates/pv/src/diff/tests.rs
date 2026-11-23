//! Tests for the diff view.

use super::cells::{Cell, spans_of};
use super::*;
use crate::git::PatchKind;
use git_core::{Kind, PatchLine};
use ratatui::style::Modifier;

/// A patch of `src/lib.rs` going from `old` to `new`.
fn patch(old: &str, new: &str) -> FilePatch {
    let hunks = git_core::hunks_of(old.as_bytes(), new.as_bytes());
    FilePatch {
        path: "src/lib.rs".into(),
        old_path: None,
        modes: None,
        kind: Kind::Modified,
        binary: false,
        hunks,
        before: old.into(),
        after: new.into(),
    }
}

/// The text of each of `lines`, with the styles dropped.
fn text(lines: &[Line]) -> Vec<String> {
    lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()).collect()
}

/// A small file before the change the tests make.
const OLD: &str = "fn main() {\n    let total = count + 1;\n    println!(\"{total}\");\n}\n";
/// The same file after the change: one word differs.
const NEW: &str = "fn main() {\n    let total = count + step;\n    println!(\"{total}\");\n}\n";

#[test]
fn rows_carry_both_line_numbers_and_the_function_context() {
    let opts = Options { width: Some(80), ..Options::default() };
    let (lines, anchors) = render(&[patch(OLD, NEW)], &Theme::default(), &opts, 100);
    let t = text(&lines);
    assert!(t.iter().any(|l| l.starts_with("  ⋯ line 1")), "{t:#?}");
    // A hunk header names the function it sits in when it starts below it.
    let long_old = format!(
        "fn first() {{\n}}\n\nfn second() {{\n{}    let a = 1;\n}}\n",
        "    // pad\n".repeat(6)
    );
    let long_new = long_old.replace("let a = 1", "let a = 2");
    let (lines, _) = render(&[patch(&long_old, &long_new)], &Theme::default(), &opts, 100);
    assert!(
        text(&lines).iter().any(|l| l.starts_with("  ⋯ line") && l.contains("fn second()")),
        "{:#?}",
        text(&lines)
    );
    // The removed row shows its old number, the added row its new one.
    assert!(t.iter().any(|l| l.contains(" 2   - ") || l.contains("2   - ")), "{t:#?}");
    assert!(t.iter().any(|l| l.contains("  2 + ")), "{t:#?}");
    assert_eq!((anchors.files.len(), anchors.hunks.len()), (1, 1));
}

#[test]
fn big_and_generated_files_fold_by_themselves_and_a_fold_can_be_toggled() {
    reset_folds();
    let opts = Options { width: Some(80), ..Options::default() };
    let mut lock = patch("a\n", "a\nb\n");
    lock.path = "Cargo.lock".into();
    let (lines, _) = render(&[lock.clone()], &Theme::default(), &opts, 100);
    let t = text(&lines);
    assert!(
        t.iter().any(|l| l.contains("folded")) && !t.iter().any(|l| l.contains("⋯ line")),
        "{t:#?}"
    );
    toggle_fold("Cargo.lock");
    let (lines, _) = render(&[lock], &Theme::default(), &opts, 100);
    assert!(text(&lines).iter().any(|l| l.contains("⋯ line")));
    // An ordinary file folds by hand.
    toggle_fold("src/lib.rs");
    let (lines, anchors) = render(&[patch(OLD, NEW)], &Theme::default(), &opts, 100);
    assert!(text(&lines).iter().any(|l| l.contains("folded")));
    assert_eq!(anchors.paths, vec!["src/lib.rs".to_string()]);
    reset_folds();
}

#[test]
fn whitespace_only_changes_are_dimmed_or_shown_as_unchanged() {
    let opts = Options { width: Some(80), ..Options::default() };
    let (old, new) = ("a\n  b\nc\n", "a\n\tb\nc\nd\n");
    set_ignore_whitespace(false);
    let (lines, _) = render(&[patch(old, new)], &Theme::default(), &opts, 100);
    let t = text(&lines);
    assert!(
        t.iter().any(|l| l.contains("- ") && l.contains("  b"))
            && t.iter().any(|l| l.contains("+ ") && l.contains("b")),
        "{t:#?}"
    );
    set_ignore_whitespace(true);
    let (lines, _) = render(&[patch(old, new)], &Theme::default(), &opts, 100);
    let t = text(&lines);
    assert!(!t.iter().any(|l| l.contains(" - ")), "the removed whitespace line is gone: {t:#?}");
    assert!(t.iter().any(|l| l.contains("+ d")), "{t:#?}");
    set_ignore_whitespace(false);
}

#[test]
fn a_line_that_does_not_fit_ends_in_a_marker() {
    let cells: Vec<Cell> = "abcdefghij".chars().map(|c| (c, Style::new())).collect();
    let spans = spans_of(&cells, Some(6), None);
    let s: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(s, "abcde…");
    let spans = spans_of(&cells, Some(10), None);
    assert_eq!(spans.iter().map(|s| s.content.as_ref()).collect::<String>(), "abcdefghij");
}
#[test]
fn the_changed_word_gets_a_stronger_background_than_the_rest_of_its_row() {
    let opts = Options { width: Some(80), ..Options::default() };
    let (lines, _) = render(&[patch(OLD, NEW)], &Theme::default(), &opts, 100);
    let added = lines
        .iter()
        .find(|l| l.spans.iter().any(|s| s.content.contains("step")))
        .expect("the added row");
    let bgs: std::collections::HashSet<_> = added.spans.iter().filter_map(|s| s.style.bg).collect();
    // Two backgrounds: the row tint and the stronger one on `step`.
    assert!(bgs.len() >= 2, "{added:?}");
    let step = added.spans.iter().find(|s| s.content.contains("step")).unwrap();
    let other = added.spans.iter().find(|s| s.content.contains("count")).unwrap();
    assert_ne!(step.style.bg, other.style.bg);
}

#[test]
fn code_is_syntax_highlighted_inside_the_diff() {
    let opts = Options { width: Some(80), ..Options::default() };
    let (lines, _) = render(&[patch(OLD, NEW)], &Theme::default(), &opts, 100);
    let colours: std::collections::HashSet<_> =
        lines.iter().flat_map(|l| &l.spans).filter_map(|s| s.style.fg).collect();
    assert!(colours.len() >= 5, "{colours:?}");
}

#[test]
fn trailing_whitespace_on_an_added_line_and_conflict_markers_are_called_out() {
    let opts = Options { width: Some(80), ..Options::default() };
    let (lines, _) =
        render(&[patch("a\nb\n", "a\nb  \n<<<<<<< HEAD\n")], &Theme::default(), &opts, 100);
    let ws = lines
        .iter()
        .find(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().contains("+ b  "))
        .expect("the b row");
    let bgs: std::collections::HashSet<_> = ws.spans.iter().filter_map(|s| s.style.bg).collect();
    assert!(bgs.len() >= 2, "{ws:?}");
    let marker = lines
        .iter()
        .find(|l| l.spans.iter().any(|s| s.content.contains("<<<<<<<")))
        .expect("the marker row");
    assert!(
        marker.spans.iter().any(|s| s.content.contains("<<<<<<<") && s.style.add_modifier.contains(Modifier::BOLD))
    );
}

#[test]
fn renames_and_mode_changes_show_in_the_file_header() {
    let mut p = patch("x\n", "y\n");
    p.kind = Kind::Renamed;
    p.old_path = Some("src/old.rs".into());
    p.modes = Some(("100644".into(), "100755".into()));
    let (lines, _) =
        render(&[p], &Theme::default(), &Options { width: Some(80), ..Options::default() }, 100);
    let t = text(&lines);
    assert!(
        t.iter()
            .any(|l| l.contains("src/old.rs → src/lib.rs") && l.contains("mode 100644 → 100755")),
        "{t:#?}"
    );
}

#[test]
fn wide_screens_draw_old_beside_new() {
    let opts = Options { width: Some(160), ..Options::default() };
    let (lines, _) = render(&[patch(OLD, NEW)], &Theme::default(), &opts, 100);
    let t = text(&lines);
    let pair = t
        .iter()
        .find(|l| l.contains("count + 1") && l.contains("count + step"))
        .expect("old and new on one row");
    assert!(pair.contains('│'), "{pair}");
    assert!(lines.iter().all(|l| {
        l.spans
            .iter()
            .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
            .sum::<usize>()
            <= 160
    }));
}

#[test]
fn anchors_find_the_next_and_previous_mark() {
    let marks = [3, 10, 20];
    assert_eq!(Anchors::next(&marks, 3), Some(10));
    assert_eq!(Anchors::next(&marks, 20), None);
    assert_eq!(Anchors::prev(&marks, 10), Some(3));
    assert_eq!(Anchors::prev(&marks, 3), None);
    assert_eq!(
        Anchors { files: vec![1], hunks: vec![2], ..Default::default() }.shifted(5),
        Anchors { files: vec![6], hunks: vec![7], ..Default::default() }
    );
}

#[test]
fn a_file_too_big_to_keep_whole_is_still_highlighted_per_hunk() {
    let mut p = patch(OLD, NEW);
    p.before.clear();
    p.after.clear();
    let (lines, _) =
        render(&[p], &Theme::default(), &Options { width: Some(80), ..Options::default() }, 100);
    let colours: std::collections::HashSet<_> =
        lines.iter().flat_map(|l| &l.spans).filter_map(|s| s.style.fg).collect();
    assert!(colours.len() >= 5, "{colours:?}");
    let _ = PatchLine { kind: PatchKind::Context, text: String::new() };
}

#[test]
fn unified_text_is_split_into_files_hunks_and_plain_text() {
    let input = "commit abc\n\n    message\n\n\
diff --git a/old.rs b/new.rs\nsimilarity index 90%\nrename from old.rs\nrename to new.rs\n\
index 1..2 100644\n--- a/old.rs\n+++ b/new.rs\n@@ -1,2 +1,2 @@ fn a\n keep\n-gone\n+came\n\
diff --git a/img.png b/img.png\nBinary files a/img.png and b/img.png differ\n";
    let chunks = unified::parse(input);
    assert!(matches!(&chunks[0], unified::Chunk::Text(t) if t.starts_with("commit abc")));
    let files: Vec<&FilePatch> = chunks
        .iter()
        .filter_map(|c| if let unified::Chunk::File(f) = c { Some(f) } else { None })
        .collect();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, "new.rs");
    assert_eq!(files[0].old_path.as_deref(), Some("old.rs"));
    assert_eq!(files[0].kind, Kind::Renamed);
    let lines = &files[0].hunks[0].lines;
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[1].kind, git_core::PatchKind::Removed);
    assert!(files[1].binary);
}

#[test]
fn unified_render_keeps_the_header_and_draws_the_patch() {
    let theme = Theme::default();
    let opts = Options { width: Some(80), height: None, git: false, focus: None };
    let out =
        text(&render_unified("commit abc\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n", &theme, &opts));
    assert_eq!(out[0], "commit abc");
    assert!(out.iter().any(|l| l.contains('b')) && out.iter().any(|l| l.contains("x")));
}
