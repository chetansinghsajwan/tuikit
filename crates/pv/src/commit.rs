//! Commit preview: header, message, and the files it changed with line counts.

use crate::diff::{self, Anchors};
use crate::git::{self, Kind, Repo};
use crate::{Error, Options};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::path::Path;
use ui::Theme;
use unicode_width::UnicodeWidthStr;

/// `YYYY-MM-DD HH:MM` in UTC from seconds since the epoch.
pub fn utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}", rem / 3600, rem % 3600 / 60)
}

fn age(secs: i64) -> String {
    let now =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(secs);
    match (now - secs).max(0) {
        0..=59 => "now".into(),
        s @ 60..=3599 => format!("{}m ago", s / 60),
        s @ 3600..=86_399 => format!("{}h ago", s / 3600),
        s @ 86_400..=2_591_999 => format!("{}d ago", s / 86_400),
        s => format!("{}mo ago", s / 2_592_000),
    }
}

pub(crate) fn sign(kind: Kind) -> char {
    match kind {
        Kind::Added => 'A',
        Kind::Deleted => 'D',
        Kind::Renamed => 'R',
        Kind::Untracked => '?',
        Kind::Conflict => 'U',
        _ => 'M',
    }
}

/// Preview of `rev` (hash, ref name or stash entry) in the repository at `root`.
pub fn render(root: &Path, rev: &str, theme: &Theme, opts: &Options) -> Result<Vec<Line<'static>>, Error> {
    render_marked(root, rev, theme, opts).map(|(lines, _)| lines)
}

/// `render`, with where the files and hunks of the patch start.
pub fn render_marked(
    root: &Path,
    rev: &str,
    theme: &Theme,
    opts: &Options,
) -> Result<(Vec<Line<'static>>, Anchors), Error> {
    render_range_marked(root, None, rev, None, theme, opts)
}

/// The difference between two commits: `base` against `rev` (with `None`, `rev` against its parent). `to_label` names the
/// `rev` side in the header when it is not a commit of its own (the work tree).
pub fn render_range_marked(
    root: &Path,
    base: Option<&str>,
    rev: &str,
    to_label: Option<&str>,
    theme: &Theme,
    opts: &Options,
) -> Result<(Vec<Line<'static>>, Anchors), Error> {
    let repo = Repo::discover(root).ok_or(Error::NotFound)?;
    let info = repo.commit(rev).map_err(|e| Error::Unsupported(e.to_string()))?;
    let changes = match base {
        Some(b) => repo.changes_between(b, rev),
        None => repo.changes(rev),
    }
    .map_err(|e| Error::Unsupported(e.to_string()))?;

    let c = &theme.colors;
    let muted = Style::new().fg(c.muted.into());
    let fg = Style::new().fg(c.fg.into());
    let label = |s: &str| Span::styled(format!("{s:<7}"), muted);
    let width = opts.width.map(usize::from);

    let mut out: Vec<Line<'static>> = Vec::new();
    if let Some(b) = base {
        // A range: the two commits being compared, not one commit's details.
        let base_info = repo.commit(b).map_err(|e| Error::Unsupported(e.to_string()))?;
        for (name, i, label_text) in [("from", &base_info, None), ("to", &info, to_label)] {
            let spans = match label_text {
                Some(text) => vec![label(name), Span::styled(text.to_string(), fg.add_modifier(Modifier::BOLD))],
                None => {
                    let mut s = vec![
                        label(name),
                        Span::styled(format!("{} ", &i.oid[..i.oid.len().min(8)]), Style::new().fg(c.warning.into())),
                    ];
                    s.extend(ui::subject::spans(
                        theme,
                        i.message.lines().next().unwrap_or(""),
                        fg.add_modifier(Modifier::BOLD),
                    ));
                    s
                }
            };
            out.push(Line::from(spans));
        }
    } else {
        // What it is first (the subject, then the rest of the message), then who, when and where it is, in one line.
        let mut lines = info.message.lines();
        let mut subject = Vec::new();
        subject.extend(ui::subject::spans(theme, lines.next().unwrap_or(""), fg.add_modifier(Modifier::BOLD)));
        out.push(Line::from(subject));
        let body: Vec<&str> = lines.skip_while(|l| l.trim().is_empty()).collect();
        if !body.is_empty() {
            out.push(Line::default());
            for l in body {
                for w in wrap(l, width.map(|w| w.saturating_sub(2))) {
                    out.push(Line::from(Span::styled(format!("  {w}"), fg)));
                }
            }
        }
        out.push(Line::default());
        let mut meta = vec![
            Span::styled(info.author.clone(), fg),
            Span::styled("  ·  ", muted),
            Span::styled(age(info.time), muted),
            Span::styled("  ·  ", muted),
            Span::styled(info.oid[..info.oid.len().min(8)].to_string(), Style::new().fg(c.warning.into())),
        ];
        // Every ref on the commit, whole: the list in `gu` cuts them to fit its column.
        let names: Vec<String> =
            repo.refs().unwrap_or_default().into_iter().filter(|r| r.oid == info.oid).map(|r| r.name).collect();
        if !names.is_empty() {
            meta.push(Span::styled("  ·  ", muted));
            meta.push(Span::styled(names.join(", "), Style::new().fg(c.accent.into())));
        }
        out.push(Line::from(meta));
    }
    out.push(Line::default());

    // The files: one row each, the folder muted and the name in the colour of its state, the counts in two
    // right-aligned columns and a bar showing how big the change is next to the others.
    const MAX_ROWS: usize = 14;
    const BAR: usize = 8;
    let (added, removed): (u32, u32) = changes.iter().fold((0, 0), |(a, r), f| (a + f.added, r + f.removed));
    out.push(Line::from(vec![
        Span::styled(
            format!("{} file{}", changes.len(), if changes.len() == 1 { "" } else { "s" }),
            fg.add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ", muted),
        Span::styled(format!("+{added}"), Style::new().fg(c.git.added.into())),
        Span::raw(" "),
        Span::styled(format!("−{removed}"), Style::new().fg(c.git.deleted.into())),
    ]));
    let biggest = changes.iter().map(|f| f.added + f.removed).max().unwrap_or(1).max(1);
    let counts = ui::tree::CountWidths::of(changes.iter().map(|f| (!f.binary).then_some((f.added, f.removed))));
    for f in changes.iter().take(MAX_ROWS) {
        let kind_color = git::kind_color(theme, f.kind);
        let (dir, name) = match f.path.rsplit_once('/') {
            Some((d, n)) => (format!("{d}/"), n.to_string()),
            None => (String::new(), f.path.clone()),
        };
        let counts_w = counts.width();
        let left = 3 + dir.width() + name.width();
        let tail = counts_w + 2 + BAR;
        let pad = width.map(|w| w.saturating_sub(left + tail).max(1)).unwrap_or(2);
        let mut spans = vec![
            Span::styled(format!(" {} ", sign(f.kind)), Style::new().fg(kind_color)),
            Span::styled(dir, muted),
            Span::styled(name, Style::new().fg(kind_color)),
            Span::raw(" ".repeat(pad)),
        ];
        spans.extend(ui::tree::line_counts(theme, (!f.binary).then_some((f.added, f.removed)), counts));
        spans.push(Span::raw("  "));
        // Filled cells in proportion to the biggest change of the commit, green and red by share.
        let total = f.added + f.removed;
        let filled = if total == 0 { 0 } else { ((total as usize * BAR).div_ceil(biggest as usize)).clamp(1, BAR) };
        let green = if total == 0 { 0 } else { ((f.added as usize * filled).div_ceil(total as usize)).min(filled) };
        spans.push(Span::styled("█".repeat(green), Style::new().fg(c.git.added.into())));
        spans.push(Span::styled("█".repeat(filled - green), Style::new().fg(c.git.deleted.into())));
        spans.push(Span::styled("░".repeat(BAR - filled), Style::new().fg(ui::mix(c.muted.into(), c.bg.into(), 40))));
        out.push(Line::from(spans));
    }
    if changes.len() > MAX_ROWS {
        let rest: u32 = changes.iter().skip(MAX_ROWS).map(|f| f.added + f.removed).sum();
        out.push(Line::from(Span::styled(
            format!("   … {} more files, {rest} lines", changes.len() - MAX_ROWS),
            muted,
        )));
    }
    let limit = opts.height.map(usize::from).unwrap_or(MAX_LINES);
    let mut anchors = Anchors::default();
    if out.len() < limit {
        let rest = limit - out.len();
        let patch = match base {
            Some(b) => repo.patch_between(b, rev),
            None => repo.patch(rev),
        };
        if let Ok(files) = patch {
            let (lines, a) = diff::render(&files, theme, opts, rest);
            anchors = a.shifted(out.len());
            out.extend(lines);
        }
    }
    out.truncate(limit);
    Ok((out, anchors))
}

/// Most lines drawn when the caller gives no height.
const MAX_LINES: usize = 4000;

#[cfg(test)]
mod tests;

/// Preview of one file's uncommitted change in the repository at `root`:
/// `side` picks staged (HEAD against the index) or unstaged (index against the
/// work tree). `rela` is relative to the work tree.
pub fn render_file_diff(
    root: &Path,
    rela: &Path,
    side: git_core::Side,
    theme: &Theme,
    opts: &Options,
) -> Result<Vec<Line<'static>>, Error> {
    render_file_diff_marked(root, rela, side, theme, opts).map(|(lines, _)| lines)
}

/// `render_file_diff`, with where the hunks start.
pub fn render_file_diff_marked(
    root: &Path,
    rela: &Path,
    side: git_core::Side,
    theme: &Theme,
    opts: &Options,
) -> Result<(Vec<Line<'static>>, Anchors), Error> {
    let repo = Repo::discover(root).ok_or(Error::NotFound)?;
    let patch = repo.diff_file(rela, side).map_err(|e| Error::Unsupported(e.to_string()))?;
    let limit = opts.height.map(usize::from).unwrap_or(MAX_LINES);
    let (mut lines, mut anchors) = diff::render(std::slice::from_ref(&patch), theme, opts, limit + 1);
    // The patch starts with a blank separator line; a single file does not need it.
    if lines.first().is_some_and(|l| l.spans.is_empty()) {
        lines.remove(0);
        anchors.files.iter_mut().for_each(|m| *m = m.saturating_sub(1));
        anchors.hunks.iter_mut().for_each(|m| *m = m.saturating_sub(1));
    }
    if !patch.binary && patch.hunks.is_empty() {
        lines.push(Line::from(Span::styled("no changes", Style::new().fg(theme.colors.muted.into()))));
    }
    lines.truncate(limit);
    Ok((lines, anchors))
}

/// The diff of `rev` for the files at `prefix` (one file, or every file below a folder; an empty prefix is the
/// whole commit's patch without its header), with where the files and hunks start.
pub fn render_files_marked(
    root: &Path,
    rev: &str,
    prefix: &str,
    theme: &Theme,
    opts: &Options,
) -> Result<(Vec<Line<'static>>, Anchors), Error> {
    let repo = Repo::discover(root).ok_or(Error::NotFound)?;
    let all = repo.patch(rev).map_err(|e| Error::Unsupported(e.to_string()))?;
    let below = format!("{}/", prefix.trim_end_matches('/'));
    let files: Vec<git_core::FilePatch> =
        all.into_iter().filter(|f| prefix.is_empty() || f.path == prefix || f.path.starts_with(&below)).collect();
    let limit = opts.height.map(usize::from).unwrap_or(MAX_LINES);
    let (mut lines, mut anchors) = diff::render(&files, theme, opts, limit + 1);
    if lines.first().is_some_and(|l| l.spans.is_empty()) {
        lines.remove(0);
        anchors.files.iter_mut().for_each(|m| *m = m.saturating_sub(1));
        anchors.hunks.iter_mut().for_each(|m| *m = m.saturating_sub(1));
    }
    if files.is_empty() {
        lines.push(Line::from(Span::styled("no changes", Style::new().fg(theme.colors.muted.into()))));
    }
    lines.truncate(limit);
    Ok((lines, anchors))
}

/// `text` broken into lines of at most `width` columns at spaces (as is when there is no width).
fn wrap(text: &str, width: Option<usize>) -> Vec<String> {
    let Some(width) = width.filter(|w| *w > 8) else { return vec![text.to_string()] };
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        if !line.is_empty() && line.width() + 1 + word.width() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    lines.push(line);
    lines
}
