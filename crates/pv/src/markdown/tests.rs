use super::*;

fn text(lines: &[Line]) -> Vec<String> {
    lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()).collect()
}

#[test]
fn conceal_finds_the_syntax_to_hide() {
    let src =
        "# Title\n\nsome **bold** and *it* and `code` and [a link](http://x.y) end\n\n- item\n";
    let hidden = |c: &Conceal| src[c.range.clone()].to_string();
    let all = conceal(src);
    let marks: Vec<String> = all.iter().map(hidden).collect();
    assert_eq!(marks, ["# ", "**", "**", "*", "*", "`", "`", "[", "](http://x.y)", "-"]);
    // The element of a mark is what the cursor has to be in to show it.
    let bold = all.iter().find(|c| hidden(c) == "**").unwrap();
    assert_eq!(&src[bold.element.clone()], "**bold**");
    let bullet = all.last().unwrap();
    assert_eq!(bullet.replace, Some("•"));
    // A heading mark is shown while the cursor is on its line.
    assert_eq!(&src[all[0].element.clone()], "# Title");
}

#[test]
fn headings_lists_quotes_and_code_lose_their_markup() {
    let md = "# Title\n\nSome *emphasis* and `code` here.\n\n- one\n- two\n\n1. first\n\n> quoted\n\n```rust\nlet x = 1;\n```\n";
    let lines = text(&render(md, &Theme::default(), 60));
    let all = lines.join("\n");
    assert!(!all.contains("# Title"), "{all}");
    // A level one heading is big text (see `h1_is_three_rows_of_blocks`), not the words.
    assert!(lines[0].contains('▀') || lines[0].contains('█'), "{all}");
    // Inline code is a pill: the code between rounded caps.
    assert!(all.contains("Some emphasis and \u{e0b6}code\u{e0b4} here."), "{all}");
    assert!(all.contains("• one") && all.contains("• two") && all.contains("1. first"), "{all}");
    assert!(all.contains("▏ quoted"), "{all}");
    assert!(all.contains("let x = 1;") && !all.contains("```"), "{all}");
}

#[test]
fn h1_is_three_rows_of_blocks() {
    let lines = text(&render("# Hi\n\nafter\n", &Theme::default(), 40));
    assert_eq!(lines[0], "█ █ ▀█▀", "{lines:#?}");
    assert_eq!(lines[1].chars().count(), 7);
    assert_eq!(lines[2].chars().count(), 7);
    assert!(lines[3].is_empty() && lines[4] == "after", "{lines:#?}");
}

#[test]
fn h1_that_does_not_fit_or_has_no_glyphs_stays_one_row() {
    let wide = text(&render("# A long heading that cannot fit\n", &Theme::default(), 20));
    assert_eq!(wide[0], "█ A long heading that cannot fit".chars().take(wide[0].chars().count()).collect::<String>(), "{wide:#?}");
    let code = text(&render("# With `code`\n", &Theme::default(), 40));
    assert!(code[0].starts_with("█ With"), "{code:#?}");
}

#[test]
fn h2_and_below_stay_one_row() {
    let lines = text(&render("## Title\n", &Theme::default(), 40));
    assert_eq!(lines[0], "▌ Title", "{lines:#?}");
}

#[test]
fn tables_are_bordered_aligned_and_fit_the_width() {
    let md = "| name | n |\n|:-----|--:|\n| alpha | 1 |\n| b | 22 |\n";
    let lines = text(&render(md, &Theme::default(), 40));
    assert_eq!(lines[0], "┌───────┬─────┐", "{lines:#?}");
    assert!(lines[1].contains("name") && lines[1].starts_with('│'), "{lines:#?}");
    assert!(lines.iter().any(|l| l == "│ alpha │   1 │"), "numbers align right: {lines:#?}");
    assert!(lines.last().unwrap().starts_with('└'));
    // Too wide: the columns shrink and the cells wrap, nothing sticks out.
    let wide = format!("| a | b |\n|---|---|\n| {} | y |\n", "word ".repeat(30));
    let lines = text(&render(&wide, &Theme::default(), 40));
    assert!(lines.iter().all(|l| l.width() <= 40), "{lines:#?}");
    assert!(lines.len() > 6, "the long cell wrapped: {lines:#?}");
}

#[test]
fn alerts_footnotes_front_matter_math_and_headings() {
    let md = "---\ntitle: T\n---\n\n# One\n\n> [!WARNING]\n> careful\n\nA note[^a] and $x^2$.\n\n[^a]: the footnote\n\n## Two\n";
    let r = render_in(md, &Theme::default(), 60, None);
    let all = text(&r.lines).join("\n");
    assert!(all.contains("title: T") && all.contains("┈"), "{all}");
    assert!(all.contains("⚠ Warning") && all.contains("▏ careful"), "{all}");
    assert!(all.contains("A note[1] and x^2."), "{all}");
    assert!(all.contains("[1] the footnote"), "{all}");
    assert_eq!(r.headings.len(), 2);
    assert!(text(&r.lines)[r.headings[1]].contains("Two"));
}

#[test]
fn code_blocks_are_highlighted_and_images_fall_back_to_their_alt_text() {
    let lines =
        render("```rust\nfn main() {}\n```\n\n![a cat](missing.png)\n", &Theme::default(), 50);
    let code = lines.iter().find(|l| l.spans.iter().any(|s| s.content.contains("fn"))).unwrap();
    let colors: std::collections::HashSet<_> =
        code.spans.iter().filter_map(|s| s.style.fg).collect();
    assert!(colors.len() > 1, "the keyword and the rest differ in color");
    assert!(text(&lines).join("\n").contains("🖼 a cat (missing.png)"));
    // A real image next to the file is drawn.
    let d = std::env::temp_dir().join(format!("pv-md-img-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 0, 0, 255]))
        .save(d.join("p.png"))
        .unwrap();
    let r = render_in("![red](p.png)\n", &Theme::default(), 50, Some(&d));
    assert!(
        r.lines.iter().any(|l| l.spans.iter().any(|s| s.content == "▀")),
        "{:?}",
        text(&r.lines)
    );
}

#[test]
fn code_blocks_are_rounded_boxes_with_the_language_inside() {
    let lines =
        text(&render("```rust\nfn main() {}\nlet a_long_line = 1;\n```\n", &Theme::default(), 40));
    assert!(lines[0].starts_with('╭') && lines[0].ends_with('╮'), "{lines:#?}");
    assert!(
        lines[1].starts_with("│ rust") && lines[1].ends_with('│'),
        "the language is inside: {lines:#?}"
    );
    assert!(lines[2].contains("fn main") && lines[3].contains("let a_long_line"));
    assert!(lines[4].starts_with('╰') && lines[4].ends_with('╯'));
    assert!(
        lines[..5].iter().all(|l| l.width() == lines[0].width()),
        "every row is as wide as the top: {lines:#?}"
    );
    // Only the inside is tinted: the frame (corners, edges, sides) is on the terminal's background,
    // so no colour shows outside the rounded corners.
    let styled = render("```rust\nfn main() {}\n```\n", &Theme::default(), 40);
    let bg = |row: usize, col: usize| styled[row].spans[col].style.bg;
    let last = |row: usize| styled[row].spans.len() - 1;
    assert!(bg(0, 0).is_none(), "the top left corner has no tint");
    assert!(bg(2, 0).is_none() && bg(2, last(2)).is_none(), "the sides have no tint");
    assert!(bg(3, 0).is_none(), "the bottom left corner has no tint");
    assert!(bg(2, 1).is_some(), "the inside is tinted");
}

#[test]
fn inline_code_is_a_pill_between_rounded_caps() {
    let r = render("use `a b` here, or `x`.\n", &Theme::default(), 60);
    let line = &r[0];
    let shown: Vec<&str> = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        shown.windows(3).any(|w| w == ["\u{e0b6}", "a b", "\u{e0b4}"]),
        "a cap, the code, a cap: {shown:?}"
    );
    assert!(shown.contains(&"x"), "{shown:?}");
    let code = line.spans.iter().find(|s| s.content == "a b").unwrap();
    assert!(code.style.bg.is_some(), "the code sits on a tint");
}

#[test]
fn paragraphs_wrap_to_the_width() {
    let md = "word ".repeat(40);
    let lines = text(&render(&md, &Theme::default(), 30));
    assert!(lines.len() > 3 && lines.iter().all(|l| l.width() <= 30), "{lines:?}");
}

#[test]
fn links_show_their_target() {
    let all = text(&render("see [the docs](https://x.y/z) now", &Theme::default(), 80)).join("\n");
    assert!(all.contains("the docs (https://x.y/z) now"), "{all}");
}
