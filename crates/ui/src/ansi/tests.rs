use super::*;

fn flat(l: &Line) -> String {
    l.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn plain_text_splits_into_lines() {
    let l = parse("one\ntwo\r\n\tthree");
    assert_eq!(l.iter().map(flat).collect::<Vec<_>>(), vec!["one", "two", "    three"]);
}

#[test]
fn colors_and_resets() {
    let l = parse("a\x1b[31mred\x1b[0m b\x1b[1;38;2;1;2;3mX\x1b[0m\n");
    assert_eq!(flat(&l[0]), "ared bX");
    assert_eq!(l[0].spans[1].style.fg, Some(Color::Red));
    assert_eq!(l[0].spans[2].style.fg, None);
    assert_eq!(l[0].spans[3].style.fg, Some(Color::Rgb(1, 2, 3)));
    assert!(l[0].spans[3].style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn indexed_background_and_unknown_sequences_are_dropped() {
    let l = parse("\x1b[48;5;200mhi\x1b[2K\x1b]0;title\x07!\x1b[m");
    assert_eq!(flat(&l[0]), "hi!");
    assert_eq!(l[0].spans[0].style.bg, Some(Color::Indexed(200)));
}
