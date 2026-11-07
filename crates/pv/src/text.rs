//! Syntax-highlighted text with a line-number gutter.

use crate::git::{self, LineKind, Repo};
use crate::{Error, Options};
use ratatui::style::Color;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::io::Read;
use std::path::Path;
use std::str::FromStr;
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color as SColor, FontStyle, ScopeSelectors, StyleModifier, Theme as SynTheme, ThemeItem,
    ThemeSettings,
};
use syntect::parsing::SyntaxSet;
use ui::Theme;
use unicode_width::UnicodeWidthStr;

/// Cap on bytes read, so a huge file never stalls a preview.
const MAX_BYTES: u64 = 1 << 20;
const TAB: &str = "    ";

fn rgb(c: ui::theme::Hex) -> SColor {
    match c.0 {
        ratatui::style::Color::Rgb(r, g, b) => SColor { r, g, b, a: 255 },
        _ => SColor { r: 255, g: 255, b: 255, a: 255 },
    }
}

/// The syntax definitions, loaded once (they are big).
pub(crate) fn syntax_set() -> &'static SyntaxSet {
    static SET: std::sync::OnceLock<SyntaxSet> = std::sync::OnceLock::new();
    SET.get_or_init(two_face::syntax::extra_newlines)
}

/// Build a syntect theme from the design tokens, so highlighting follows them.
pub(crate) fn syntax_theme(t: &Theme) -> SynTheme {
    let s = &t.colors.syntax;
    let plain = FontStyle::empty();
    let rules: &[(&str, ui::theme::Hex, FontStyle)] = &[
        ("comment", s.comment, plain),
        ("keyword, storage", s.keyword, plain),
        ("entity.name.function, support.function, meta.function-call", s.function, plain),
        ("string", s.string, plain),
        ("constant.numeric", s.number, plain),
        (
            "entity.name.type, entity.name.class, support.type, support.class, storage.type",
            s.type_,
            plain,
        ),
        ("constant, support.constant", s.constant, plain),
        ("variable", s.variable, plain),
        ("keyword.operator", s.operator, plain),
        // Markdown: headings, emphasis, code, links, lists and quotes.
        (
            "markup.heading, entity.name.section, punctuation.definition.heading",
            s.function,
            FontStyle::BOLD,
        ),
        ("markup.bold", s.constant, FontStyle::BOLD),
        ("markup.italic", s.variable, FontStyle::ITALIC),
        ("markup.raw, markup.inline.raw, markup.fenced_code, markup.raw.block", s.string, plain),
        (
            "markup.underline.link, string.other.link, meta.link, markup.underline",
            s.type_,
            FontStyle::UNDERLINE,
        ),
        (
            "punctuation.definition.list_item, punctuation.definition.list, markup.list punctuation",
            s.keyword,
            plain,
        ),
        ("markup.quote, punctuation.definition.blockquote", s.comment, plain),
    ];
    let scopes = rules
        .iter()
        .filter_map(|(sel, c, font)| {
            Some(ThemeItem {
                scope: ScopeSelectors::from_str(sel).ok()?,
                style: StyleModifier {
                    foreground: Some(rgb(*c)),
                    font_style: if font.is_empty() { None } else { Some(*font) },
                    ..Default::default()
                },
            })
        })
        .collect();
    SynTheme {
        settings: ThemeSettings { foreground: Some(rgb(t.colors.fg)), ..Default::default() },
        scopes,
        ..Default::default()
    }
}

fn is_binary(buf: &[u8]) -> bool {
    buf.iter().take(8000).any(|&b| b == 0)
}

/// Per-line git marks (index = 0-based line) from changes vs HEAD.
fn line_marks(path: &Path, total: usize) -> Vec<Option<(LineKind, char)>> {
    let mut marks = vec![None; total];
    let Some(repo) = Repo::discover(path) else { return marks };
    let Ok(changes) = repo.line_changes(path) else { return marks };
    for c in changes {
        match c.kind {
            LineKind::Deleted => {
                // Removed lines have no row: mark the line after (or the last one).
                if total == 0 {
                    continue;
                }
                let (row, ch) = if (c.start as usize) < total {
                    (c.start as usize, '▔')
                } else {
                    (total - 1, '▁')
                };
                marks[row].get_or_insert((LineKind::Deleted, ch));
            }
            kind => {
                for l in c.start..=c.end {
                    if let Some(m) = marks.get_mut(l as usize - 1) {
                        *m = Some((kind, '▎'));
                    }
                }
            }
        }
    }
    marks
}

pub fn render(path: &Path, theme: &Theme, opts: &Options) -> Result<Vec<Line<'static>>, Error> {
    let mut buf = Vec::new();
    std::fs::File::open(path)?.take(MAX_BYTES).read_to_end(&mut buf)?;
    render_bytes(path, &buf, theme, opts, opts.git)
}

/// Render `buf` as the contents of `path` (which only picks the syntax and,
/// with `git_marks`, the file to compare against `HEAD`). Used for text that
/// is not on disk as shown, e.g. an editor buffer with unsaved changes.
pub fn render_bytes(
    path: &Path,
    buf: &[u8],
    theme: &Theme,
    opts: &Options,
    git_marks: bool,
) -> Result<Vec<Line<'static>>, Error> {
    let buf = &buf[..buf.len().min(MAX_BYTES as usize)];
    if is_binary(buf) {
        return Err(Error::Unsupported("binary".into()));
    }
    let text = String::from_utf8_lossy(buf);

    let syntaxes: &SyntaxSet = syntax_set();
    // `flake.lock` is JSON whatever its extension says.
    let by_name = if path.file_name().is_some_and(|n| n == "flake.lock") {
        syntaxes.find_syntax_by_name("JSON")
    } else {
        None
    };
    let syntax = by_name
        .or_else(|| syntaxes.find_syntax_for_file(path).ok().flatten())
        .or_else(|| text.lines().next().and_then(|l| syntaxes.find_syntax_by_first_line(l)))
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let syn_theme = syntax_theme(theme);
    let mut hl = HighlightLines::new(syntax, &syn_theme);

    let limit = opts.height.map(usize::from).unwrap_or(usize::MAX);
    let total = text.lines().count();
    let digits = total.max(1).to_string().len().max(3); // room for rows up to 999
    let gutter = Style::new().fg(theme.colors.muted.into());
    let marks = if git_marks && opts.git { line_marks(path, total) } else { Vec::new() };
    let (depths, unit) = indent_depths(&text);
    // With a focus line and a height, show a window centered on it.
    let start = match (opts.focus, opts.height) {
        (Some(f), Some(_)) => {
            let centered = (f as usize).saturating_sub(1).saturating_sub(limit / 2);
            centered.min(total.saturating_sub(limit))
        }
        _ => 0,
    };

    let mut out = Vec::new();
    for (i, raw) in syntect::util::LinesWithEndings::from(&text).enumerate() {
        if i >= start.saturating_add(limit) {
            break;
        }
        // Lines before the window still feed the highlighter's state.
        let regions =
            hl.highlight_line(raw, syntaxes).map_err(|e| Error::Unsupported(e.to_string()))?;
        if i < start {
            continue;
        }
        let mark = marks.get(i).copied().flatten();
        let mut spans = Vec::new();
        if opts.git {
            // Sign column: a bar in the change color, blank otherwise.
            spans.push(match mark {
                Some((kind, ch)) => {
                    Span::styled(ch.to_string(), Style::new().fg(git::line_color(theme, kind)))
                }
                None => Span::raw(" "),
            });
        }
        spans.push(Span::styled(format!("{:>digits$} ", i + 1), gutter));
        let mut code: Vec<Span<'static>> = Vec::new();
        for (st, piece) in regions {
            let piece = piece.trim_end_matches(['\n', '\r']).replace('\t', TAB);
            if piece.is_empty() {
                continue;
            }
            let fg = st.foreground;
            let mut style = Style::new().fg(Color::Rgb(fg.r, fg.g, fg.b));
            if st.font_style.contains(FontStyle::BOLD) {
                style = style.add_modifier(Modifier::BOLD);
            }
            if st.font_style.contains(FontStyle::ITALIC) {
                style = style.add_modifier(Modifier::ITALIC);
            }
            if st.font_style.contains(FontStyle::UNDERLINE) {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            code.push(Span::styled(piece, style));
        }
        // Indent guides: a thin line at each level of the line's indentation (a blank line takes its neighbors').
        let depth = depths.get(i).copied().unwrap_or(0);
        if code.is_empty() && depth > 0 {
            code.push(Span::raw(" ".repeat(depth)));
        }
        spans.extend(ui::tree::mark_indent(theme, code, depth, unit));
        // The focus line wins over a git tint; both span the full width.
        let row_bg = if opts.focus == Some(i as u32 + 1) {
            Some(Color::from(theme.colors.selection))
        } else if let Some((kind @ (LineKind::Added | LineKind::Modified), _)) = mark {
            Some(git::tint(theme, git::line_color(theme, kind)))
        } else {
            None
        };
        if let Some(bg) = row_bg {
            let used: usize = spans.iter().map(|s| s.content.width()).sum();
            if let Some(w) = opts.width {
                spans.push(Span::raw(" ".repeat((w as usize).saturating_sub(used))));
            }
            for s in &mut spans {
                s.style = s.style.bg(bg);
            }
        }
        out.push(Line::from(spans));
    }
    Ok(out)
}

/// `code` highlighted as the language `lang` (a name or an extension; plain when unknown), by line.
pub(crate) fn highlight_code(code: &str, lang: &str, theme: &Theme) -> Vec<Vec<(Style, String)>> {
    use syntect::util::LinesWithEndings;
    let syntaxes = syntax_set();
    let syntax =
        syntaxes.find_syntax_by_token(lang).unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let syn_theme = syntax_theme(theme);
    let mut hl = HighlightLines::new(syntax, &syn_theme);
    LinesWithEndings::from(code)
        .map(|line| {
            hl.highlight_line(line, syntaxes)
                .unwrap_or_default()
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
                    (style, piece.trim_end_matches(['\n', '\r']).replace('\t', "    "))
                })
                .collect()
        })
        .collect()
}

/// The indentation of every line of `text` in cells (a tab is one `TAB`), and the indent unit of the file for
/// the guides: the smallest indentation of two or more cells, 4 when there is none. A blank line takes the smaller
/// of its neighbors' indentation, so the guides run through it.
fn indent_depths(text: &str) -> (Vec<usize>, usize) {
    let own: Vec<Option<usize>> = text
        .lines()
        .map(|l| {
            if l.trim().is_empty() {
                None
            } else {
                Some(
                    l.chars()
                        .take_while(|c| *c == ' ' || *c == '\t')
                        .map(|c| if c == '\t' { TAB.len() } else { 1 })
                        .sum(),
                )
            }
        })
        .collect();
    let unit =
        own.iter().take(400).flatten().copied().filter(|n| *n >= 2).min().unwrap_or(4).clamp(2, 8);
    (ui::tree::indent_levels(&own), unit)
}
