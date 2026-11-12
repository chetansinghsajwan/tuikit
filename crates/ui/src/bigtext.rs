//! Large text from block characters: a 3x5 pixel font drawn with half blocks, so a letter is three
//! terminal rows tall and three columns wide. It needs no special font, so it works in every
//! terminal; text it has no glyph for is left to the caller to show normally.

/// Rows a big line takes.
pub const HEIGHT: usize = 3;

/// The pixel rows (`1` is set) of `c`, 5 rows tall; lowercase letters use the capital glyphs.
fn glyph(c: char) -> Option<&'static [&'static str; 5]> {
    Some(match c.to_ascii_uppercase() {
        'A' => &["010", "101", "111", "101", "101"],
        'B' => &["110", "101", "110", "101", "110"],
        'C' => &["011", "100", "100", "100", "011"],
        'D' => &["110", "101", "101", "101", "110"],
        'E' => &["111", "100", "110", "100", "111"],
        'F' => &["111", "100", "110", "100", "100"],
        'G' => &["011", "100", "101", "101", "011"],
        'H' => &["101", "101", "111", "101", "101"],
        'I' => &["111", "010", "010", "010", "111"],
        'J' => &["001", "001", "001", "101", "010"],
        'K' => &["101", "101", "110", "101", "101"],
        'L' => &["100", "100", "100", "100", "111"],
        'M' => &["101", "111", "111", "101", "101"],
        'N' => &["110", "101", "101", "101", "101"],
        'O' => &["010", "101", "101", "101", "010"],
        'P' => &["110", "101", "110", "100", "100"],
        'Q' => &["010", "101", "101", "110", "011"],
        'R' => &["110", "101", "110", "101", "101"],
        'S' => &["011", "100", "010", "001", "110"],
        'T' => &["111", "010", "010", "010", "010"],
        'U' => &["101", "101", "101", "101", "111"],
        'V' => &["101", "101", "101", "101", "010"],
        'W' => &["101", "101", "111", "111", "101"],
        'X' => &["101", "101", "010", "101", "101"],
        'Y' => &["101", "101", "010", "010", "010"],
        'Z' => &["111", "001", "010", "100", "111"],
        '0' => &["111", "101", "101", "101", "111"],
        '1' => &["010", "110", "010", "010", "111"],
        '2' => &["110", "001", "010", "100", "111"],
        '3' => &["110", "001", "010", "001", "110"],
        '4' => &["101", "101", "111", "001", "001"],
        '5' => &["111", "100", "110", "001", "110"],
        '6' => &["011", "100", "111", "101", "111"],
        '7' => &["111", "001", "010", "010", "010"],
        '8' => &["111", "101", "111", "101", "111"],
        '9' => &["111", "101", "111", "001", "110"],
        ' ' => &["00", "00", "00", "00", "00"],
        '.' => &["0", "0", "0", "0", "1"],
        ',' => &["0", "0", "0", "1", "1"],
        '!' => &["1", "1", "1", "0", "1"],
        ':' => &["0", "1", "0", "1", "0"],
        '\'' => &["1", "1", "0", "0", "0"],
        '?' => &["110", "001", "010", "000", "010"],
        '-' => &["000", "000", "111", "000", "000"],
        '+' => &["000", "010", "111", "010", "000"],
        '_' => &["000", "000", "000", "000", "111"],
        '/' => &["001", "001", "010", "100", "100"],
        '(' => &["01", "10", "10", "10", "01"],
        ')' => &["10", "01", "01", "01", "10"],
        _ => return None,
    })
}

/// `text` as `HEIGHT` rows of block characters, one empty column between letters, or `None` when it
/// has a character without a glyph.
pub fn render(text: &str) -> Option<[String; HEIGHT]> {
    let glyphs: Vec<_> = text.chars().map(glyph).collect::<Option<_>>()?;
    // Five pixel rows, each the glyphs side by side with a gap.
    let mut pixels = vec![Vec::<bool>::new(); 5];
    for (i, g) in glyphs.iter().enumerate() {
        for (row, bits) in g.iter().enumerate() {
            if i > 0 {
                pixels[row].push(false);
            }
            pixels[row].extend(bits.chars().map(|b| b == '1'));
        }
    }
    // A text row is two pixel rows: the upper and lower half blocks.
    let width = pixels[0].len();
    let row = |top: usize, bottom: Option<usize>| -> String {
        (0..width)
            .map(|x| match (pixels[top][x], bottom.is_some_and(|b| pixels[b][x])) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            })
            .collect()
    };
    Some([row(0, Some(1)), row(2, Some(3)), row(4, None)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_three_rows_of_blocks() {
        let rows = render("Hi").unwrap();
        // H and I are three columns each, one column between them.
        assert!(rows.iter().all(|r| r.chars().count() == 7), "{rows:?}");
        assert_eq!(rows[0], "█ █ ▀█▀");
    }

    #[test]
    fn lowercase_uses_the_capital_glyphs() {
        assert_eq!(render("title"), render("TITLE"));
    }

    #[test]
    fn a_character_without_a_glyph_gives_none() {
        assert!(render("a `b`").is_none());
        assert!(render("é").is_none());
    }
}
