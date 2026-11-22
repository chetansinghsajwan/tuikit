//! Tests for the clipboard history store and rows.

use super::*;

/// The file format round-trips, including multi-line text.
#[test]
fn history_round_trips() {
    let entries =
        vec![Entry { time: 1, text: "one".into() }, Entry { time: 2, text: "two\nlines".into() }];
    let back = parse(&serialize(&entries));
    assert_eq!(back.len(), 2);
    assert_eq!(back[1].text, "two\nlines");
    assert_eq!(back[1].time, 2);
}

/// A repeated copy moves to the newest place instead of being listed twice.
#[test]
fn push_dedups_and_moves_to_newest() {
    let mut e = Vec::new();
    push(&mut e, "a", 1);
    push(&mut e, "b", 2);
    push(&mut e, "a", 3);
    let texts: Vec<_> = e.iter().map(|x| x.text.as_str()).collect();
    assert_eq!(texts, ["b", "a"]);
}

/// Blank copies are not recorded.
#[test]
fn push_skips_blank() {
    let mut e = Vec::new();
    push(&mut e, "  \n", 1);
    assert!(e.is_empty());
}

/// The oldest entries go once the cap is passed.
#[test]
fn push_caps_the_history() {
    let mut e = Vec::new();
    for i in 0..MAX_ENTRIES + 5 {
        push(&mut e, &i.to_string(), 1);
    }
    assert_eq!(e.len(), MAX_ENTRIES);
    assert_eq!(e[0].text, "5");
}

/// A multi-line copy is one row.
#[test]
fn row_is_one_line() {
    assert_eq!(row("  a\n  b \n"), "a b");
}
