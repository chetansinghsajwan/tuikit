//! Small drawing helpers: ref pills, author colors, widths.

use super::*;

/// Width of the hash column of the Commits table.
pub(super) const HASH_W: usize = 8;

/// The widest an author name column gets.
pub(super) const AUTHOR_COL: usize = 18;

/// Cells a ref takes as a pill (the caps, the name, the branch icon of the checked-out one, a space).
pub(super) fn ref_width(r: &RefInfo, remotes: &[String]) -> usize {
    pill_text(r, remotes).chars().count() + 3
}

/// The text of a pill: the name, with the remotes that point at the same commit under the same name
/// (`main · origin`), and the branch icon on the checked-out branch.
pub(super) fn pill_text(r: &RefInfo, remotes: &[String]) -> String {
    let mut text = if r.is_head { format!("\u{e0a0} {}", r.name) } else { r.name.clone() };
    if !remotes.is_empty() {
        text.push_str(&format!(" · {}", remotes.join(", ")));
    }
    text
}

/// The refs at one commit as pills: a branch and its remote branches (`origin/main`) share a pill.
pub(super) fn pills(refs: &[RefInfo]) -> Vec<(RefInfo, Vec<String>)> {
    let mut out: Vec<(RefInfo, Vec<String>)> = Vec::new();
    let mut used = vec![false; refs.len()];
    for (i, r) in refs.iter().enumerate() {
        if r.kind == RefKind::Remote {
            continue;
        }
        let suffix = format!("/{}", r.name);
        let mut remotes = Vec::new();
        if r.kind == RefKind::Branch {
            for (j, o) in refs.iter().enumerate() {
                if o.kind == RefKind::Remote && o.name.ends_with(&suffix) {
                    remotes.push(o.name[..o.name.len() - suffix.len()].to_string());
                    used[j] = true;
                }
            }
        }
        used[i] = true;
        out.push((r.clone(), remotes));
    }
    for (j, o) in refs.iter().enumerate() {
        if !used[j] {
            out.push((o.clone(), Vec::new()));
        }
    }
    out
}

/// A branch or tag name as a pill with rounded ends. The checked-out branch is solid with the
/// branch icon; the others are a tint of their colour (branches blue, remotes grey, tags yellow).
pub(super) fn ref_pill(theme: &Theme, r: &RefInfo, remotes: &[String]) -> Vec<Span<'static>> {
    use ratatui::style::Color;
    let c = &theme.colors;
    let color = match (r.is_head, r.kind) {
        (true, _) => Color::from(c.success),
        (_, RefKind::Branch) => Color::from(c.info),
        (_, RefKind::Remote) => Color::from(c.muted),
        (_, RefKind::Tag) => Color::from(c.warning),
    };
    let (bg, fg, text) = if r.is_head {
        (color, Color::from(c.bg), pill_text(r, remotes))
    } else {
        (ui::mix(color, Color::from(c.bg), 30), color, pill_text(r, remotes))
    };
    let mut body = Style::new().fg(fg).bg(bg);
    if r.is_head {
        body = body.add_modifier(Modifier::BOLD);
    }
    let cap = Style::new().fg(bg);
    vec![
        Span::styled("\u{e0b6}", cap),
        Span::styled(text, body),
        Span::styled("\u{e0b4}", cap),
        Span::raw(" "),
    ]
}

/// The author as two letters: the first letters of the first two words (`Chetan Singh Sajwan` ->
/// `CS`), or the first two letters of a single name (`root` -> `ro`).
pub(super) fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let letters: Vec<char> = match words.as_slice() {
        [] => vec!['?'],
        [one] => one.chars().take(2).collect(),
        [a, b, ..] => a.chars().take(1).chain(b.chars().take(1)).collect(),
    };
    letters.into_iter().collect()
}

/// A stable color per author, from the theme's accents, so one person always looks the same.
pub(super) fn author_color(theme: &Theme, name: &str) -> ratatui::style::Color {
    let c = &theme.colors;
    let palette = [
        c.success,
        c.warning,
        c.info,
        c.accent,
        c.git.untracked,
        c.git.renamed,
        c.git.modified,
        c.git.conflict,
    ];
    let hash = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33).wrapping_add(u32::from(b)));
    palette[hash as usize % palette.len()].into()
}

/// `now`, `5m ago`, `4h ago`, ... for a commit time in seconds since the epoch.
pub(super) fn ago(time: i64) -> String {
    ui::time::ago(time)
}
