//! Reads a unified diff (`git diff`, `git show`, `git log -p`, `diff -u`) from text, so a pager can draw it
//! the way the patch views do. Lines outside any file diff (a commit header, a message) stay plain text.

use super::render;
use crate::Options;
use crate::git::PatchKind;
use git_core::{FilePatch, Hunk, Kind, PatchLine};
use ratatui::text::Line;

/// A run of the input: plain text, or one file's patch.
pub(super) enum Chunk {
    Text(String),
    File(FilePatch),
}

/// `text` without terminal color escapes (git colors its output for a pager).
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            for n in chars.by_ref() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The `(start, len)` of one side of a hunk header, e.g. `12,5` or `7` (a length of 1).
fn range(side: &str) -> Option<(u32, u32)> {
    let side = side.trim_start_matches(['-', '+']);
    match side.split_once(',') {
        Some((s, l)) => Some((s.parse().ok()?, l.parse().ok()?)),
        None => Some((side.parse().ok()?, 1)),
    }
}

/// The hunk a `@@ -a,b +c,d @@` header starts, with no lines yet.
fn hunk_header(line: &str) -> Option<Hunk> {
    let mut parts = line.strip_prefix("@@ ")?.split(' ');
    let (old_start, old_len) = range(parts.next()?)?;
    let (new_start, new_len) = range(parts.next()?)?;
    Some(Hunk { old_start, old_len, new_start, new_len, lines: Vec::new() })
}

/// A path from a `---` / `+++` line: without the `a/` or `b/` prefix and a trailing tab, none for `/dev/null`.
fn header_path(rest: &str) -> Option<String> {
    let path = rest.split('\t').next().unwrap_or(rest).trim();
    if path == "/dev/null" {
        return None;
    }
    Some(path.strip_prefix("a/").or_else(|| path.strip_prefix("b/")).unwrap_or(path).to_string())
}

fn empty_file(path: &str) -> FilePatch {
    FilePatch {
        path: path.to_string(),
        old_path: None,
        modes: None,
        kind: Kind::Modified,
        binary: false,
        hunks: Vec::new(),
        before: String::new(),
        after: String::new(),
    }
}

/// Splits `text` into plain text and file patches.
pub(super) fn parse(text: &str) -> Vec<Chunk> {
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut file: Option<FilePatch> = None;
    // Lines of the current hunk still to come on the old and the new side.
    let (mut old_left, mut new_left) = (0u32, 0u32);
    let (mut old_mode, mut new_mode) = (None::<String>, None::<String>);

    let finish = |file: &mut Option<FilePatch>,
                  chunks: &mut Vec<Chunk>,
                  om: &mut Option<String>,
                  nm: &mut Option<String>| {
        if let Some(mut f) = file.take() {
            if let (Some(a), Some(b)) = (om.take(), nm.take())
                && a != b
            {
                f.modes = Some((a, b));
            }
            chunks.push(Chunk::File(f));
        }
        *om = None;
        *nm = None;
    };

    for line in text.lines() {
        if old_left > 0 || new_left > 0 {
            let hunk = file.as_mut().and_then(|f| f.hunks.last_mut());
            if let Some(h) = hunk {
                let (kind, body) = match line.chars().next() {
                    Some('+') => (PatchKind::Added, &line[1..]),
                    Some('-') => (PatchKind::Removed, &line[1..]),
                    Some('\\') => continue,
                    Some(' ') => (PatchKind::Context, &line[1..]),
                    // A context line whose leading space was stripped.
                    _ => (PatchKind::Context, line),
                };
                match kind {
                    PatchKind::Added => new_left = new_left.saturating_sub(1),
                    PatchKind::Removed => old_left = old_left.saturating_sub(1),
                    PatchKind::Context => {
                        old_left = old_left.saturating_sub(1);
                        new_left = new_left.saturating_sub(1);
                    }
                }
                h.lines.push(PatchLine { kind, text: body.to_string() });
                continue;
            }
        }
        if let Some(rest) = line.strip_prefix("diff --git ") {
            finish(&mut file, &mut chunks, &mut old_mode, &mut new_mode);
            // `a/x b/x`: the path is the second half (paths with spaces are rare and fall back to the whole text).
            let path = rest.rsplit_once(" b/").map_or(rest, |(_, p)| p);
            file = Some(empty_file(path));
        } else if let Some(h) = hunk_header(line) {
            // A plain `diff -u` has no `diff --git` line: its `---` / `+++` lines start the file.
            let f = file.get_or_insert_with(|| empty_file("?"));
            old_left = h.old_len;
            new_left = h.new_len;
            f.hunks.push(h);
        } else if let Some(rest) = line.strip_prefix("--- ") {
            if file.as_ref().is_none_or(|f| !f.hunks.is_empty()) {
                finish(&mut file, &mut chunks, &mut old_mode, &mut new_mode);
                file = Some(empty_file(&header_path(rest).unwrap_or_default()));
            }
            if let Some(f) = file.as_mut()
                && header_path(rest).is_none()
            {
                f.kind = Kind::Added;
            }
        } else if let Some(rest) = line.strip_prefix("+++ ") {
            if let Some(f) = file.as_mut() {
                match header_path(rest) {
                    Some(p) => f.path = p,
                    None => f.kind = Kind::Deleted,
                }
            }
        } else if let Some(f) = file.as_mut() {
            if line.starts_with("new file mode") {
                f.kind = Kind::Added;
            } else if line.starts_with("deleted file mode") {
                f.kind = Kind::Deleted;
            } else if let Some(m) = line.strip_prefix("old mode ") {
                old_mode = Some(m.to_string());
            } else if let Some(m) = line.strip_prefix("new mode ") {
                new_mode = Some(m.to_string());
            } else if let Some(p) = line.strip_prefix("rename from ") {
                f.old_path = Some(p.to_string());
                f.kind = Kind::Renamed;
            } else if let Some(p) = line.strip_prefix("rename to ") {
                f.path = p.to_string();
            } else if line.starts_with("Binary files") || line.starts_with("GIT binary patch") {
                f.binary = true;
            } else if !(line.starts_with("index ")
                || line.starts_with("similarity index ")
                || line.starts_with("dissimilarity index "))
            {
                // Anything else ends the file diff (the next commit's header in `git log -p`).
                finish(&mut file, &mut chunks, &mut old_mode, &mut new_mode);
                push_text(&mut chunks, line);
            }
        } else {
            push_text(&mut chunks, line);
        }
    }
    finish(&mut file, &mut chunks, &mut old_mode, &mut new_mode);
    chunks
}

fn push_text(chunks: &mut Vec<Chunk>, line: &str) {
    match chunks.last_mut() {
        Some(Chunk::Text(t)) => {
            t.push('\n');
            t.push_str(line);
        }
        _ => chunks.push(Chunk::Text(line.to_string())),
    }
}

/// The unified diff in `text` drawn like the patch views draw it; text between file diffs is kept as is.
pub fn render_unified(text: &str, cx: &ui::Context, opts: &Options) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut files: Vec<FilePatch> = Vec::new();
    let flush = |files: &mut Vec<FilePatch>, out: &mut Vec<Line<'static>>| {
        if !files.is_empty() {
            let (lines, _) = render(files, cx, opts, usize::MAX);
            out.extend(lines);
            files.clear();
        }
    };
    for chunk in parse(&strip_ansi(text)) {
        match chunk {
            Chunk::File(f) => files.push(f),
            Chunk::Text(t) => {
                flush(&mut files, &mut out);
                out.extend(t.lines().map(|l| Line::raw(l.to_string())));
            }
        }
    }
    flush(&mut files, &mut out);
    out
}
