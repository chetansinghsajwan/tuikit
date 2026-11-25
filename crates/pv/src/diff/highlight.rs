use crate::text::{syntax_set, syntax_theme};
use ratatui::style::{Color, Modifier, Style};
use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;
use syntect::util::LinesWithEndings;

/// The highlighted pieces of each line of a text: `(style, text)`.
pub(super) type Highlighted = Vec<Vec<(Style, String)>>;

/// Highlight `text` as the language of `path`, line by line.
pub(super) fn highlight(text: &str, path: &str, cx: &ui::Context, upto: usize) -> Highlighted {
    highlight_window(text, path, cx, 0, upto)
}

/// How many lines before a window are parsed (and not kept) so the window starts in the right state.
pub(super) const LEAD_IN: usize = 10;

/// Lines longer than this are not highlighted (Markdown prose and minified files take the highlighter milliseconds a line).
pub(super) const LONG_LINE: usize = 240;

/// How many lines one rendering highlights at most; the rest of a huge diff is drawn plain.
pub(super) const HIGHLIGHT_BUDGET: usize = 1500;

thread_local! {
    /// How many lines are still allowed to be highlighted in the rendering under way.
    static BUDGET: std::cell::Cell<usize> = const { std::cell::Cell::new(HIGHLIGHT_BUDGET) };
}

/// Highlight lines `from..upto` of `text`: the lines before are only parsed from `LEAD_IN` lines back, and come
/// out empty. Highlighting is by far the slowest part of a diff, so it covers what a hunk shows and no more.
pub(super) fn highlight_window(
    text: &str,
    path: &str,
    cx: &ui::Context,
    from: usize,
    upto: usize,
) -> Highlighted {
    let theme = cx.theme;
    let start = from.saturating_sub(LEAD_IN);
    let syntaxes = syntax_set();
    let name = std::path::Path::new(path);
    let syntax = syntaxes
        .find_syntax_for_file(name)
        .ok()
        .flatten()
        .or_else(|| text.lines().next().and_then(|l| syntaxes.find_syntax_by_first_line(l)))
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let theme = syntax_theme(theme);
    let mut hl = HighlightLines::new(syntax, &theme);
    let mut out: Vec<Vec<(Style, String)>> = vec![Vec::new(); start];
    for (n, line) in
        LinesWithEndings::from(text).enumerate().skip(start).take(upto.saturating_sub(start))
    {
        let spent = BUDGET.with(|b| {
            let left = b.get();
            b.set(left.saturating_sub(1));
            left == 0
        });
        if spent || line.len() > LONG_LINE {
            out.push(Vec::new());
            continue;
        }
        let pieces = hl.highlight_line(line, syntaxes).unwrap_or_default();
        if n < from {
            out.push(Vec::new());
            continue;
        }
        out.push(
            pieces
                .into_iter()
                .map(|(st, piece)| {
                    let mut style = Style::new().fg(Color::Rgb(
                        st.foreground.r,
                        st.foreground.g,
                        st.foreground.b,
                    ));
                    if st.font_style.contains(FontStyle::BOLD) {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if st.font_style.contains(FontStyle::ITALIC) {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                    (style, piece.trim_end_matches(['\n', '\r']).to_string())
                })
                .collect(),
        );
    }
    out
}

/// Give the rendering that starts now its whole highlight budget.
pub(super) fn reset_budget() {
    BUDGET.with(|b| b.set(HIGHLIGHT_BUDGET));
}
