//! Directory tree: everything when it fits (down to four levels), else two entries
//! per folder with a `… N more` marker, as deep as the room allows. A folder that
//! holds one folder reads as one row (`a/b/c`); empty folders go last and several in
//! a row become one `· N empty folders` line. Right column: permissions + age.

use crate::git::{self, Kind, Repo, Status};
use crate::{Error, Options};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use ui::Theme;
use unicode_width::UnicodeWidthStr;

struct Entry {
    name: String,
    is_dir: bool,
    mode: u32,
    mtime: Option<SystemTime>,
    path: std::path::PathBuf,
}

fn entries(dir: &Path) -> Vec<Entry> {
    let mut v: Vec<Entry> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let md = e.metadata().ok()?;
            #[cfg(unix)]
            let mode = std::os::unix::fs::PermissionsExt::mode(&md.permissions());
            #[cfg(not(unix))]
            let mode = 0;
            Some(Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                is_dir: md.is_dir(),
                mode,
                mtime: md.modified().ok(),
                path: e.path(),
            })
        })
        .collect();
    v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    v
}

fn relative(t: Option<SystemTime>) -> String {
    let Some(t) = t else { return "-".into() };
    let secs = SystemTime::now().duration_since(t).map(|d| d.as_secs()).unwrap_or(0);
    match secs {
        0..=59 => "now".into(),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86399 => format!("{}h ago", secs / 3600),
        86400..=2_591_999 => format!("{}d ago", secs / 86400),
        _ => format!("{}mo ago", secs / 2_592_000),
    }
}

fn perm_spans(mode: u32, theme: &Theme) -> Vec<Span<'static>> {
    let c = &theme.colors;
    let mut out = Vec::new();
    for i in 0..9 {
        let bit = 8 - i;
        let set = mode & (1 << bit) != 0;
        let ch = ['r', 'w', 'x'][i % 3];
        let (text, color) = if !set {
            ('-', c.muted)
        } else if i >= 3 {
            (ch, c.muted)
        } else {
            (ch, match ch { 'r' => c.success, 'w' => c.warning, _ => c.error })
        };
        out.push(Span::styled(text.to_string(), Style::new().fg(color.into())));
        if i == 2 || i == 5 {
            out.push(Span::raw(" "));
        }
    }
    out
}

fn row(prefix: &str, e: &Entry, opts: &Options, state: Option<(char, Kind)>, theme: &Theme, note: Option<&str>) -> Line<'static> {
    let width = opts.width;
    let c = &theme.colors;
    let name = e.name.clone();
    let icon = if e.is_dir { ui::icons::dir(&e.name, false) } else { ui::icons::file(&e.name, e.mode & 0o111 != 0) };
    let mut name_style = if e.is_dir {
        Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(c.fg.into())
    };
    if let Some((_, kind)) = state {
        name_style = name_style.fg(git::kind_color(theme, kind));
    }
    let mut spans = vec![
        Span::styled(prefix.to_string(), Style::new().fg(c.muted.into())),
        Span::styled(format!("{} ", icon.glyph), Style::new().fg(icon.color)),
        Span::styled(name.clone(), name_style),
    ];
    if let Some(note) = note {
        spans.push(Span::styled(format!("  {note}"), Style::new().fg(c.muted.into())));
    }
    if let Some(w) = width {
        let mut right = perm_spans(e.mode, theme);
        right.push(Span::styled(format!("  {:>8}", relative(e.mtime)), Style::new().fg(c.muted.into())));
        if opts.git {
            right.push(Span::raw(" "));
            right.push(match state {
                Some((ch, kind)) => Span::styled(ch.to_string(), Style::new().fg(git::kind_color(theme, kind))),
                None => Span::raw(" "),
            });
        }
        let right_w: usize = right.iter().map(|s| s.content.width()).sum();
        let left_w = prefix.width() + 2 + name.width() + note.map_or(0, |n| n.width() + 2);
        let pad = (w as usize).saturating_sub(left_w + right_w + 2).max(1);
        spans.push(Span::raw(" ".repeat(pad)));
        spans.extend(right);
    }
    Line::from(spans)
}

/// Levels shown when there is room: the folder's entries are level 1.
const MAX_DEPTH: usize = 4;
/// Entries shown per folder (below the top level) when the preview is too short for everything.
const CAP: usize = 2;
/// A folder is read up to this many entries; more are reported as `N+ more`.
const READ_CAP: usize = 200;
/// Entries loaded for one preview, so a huge tree stays instant.
const NODE_CAP: usize = 3000;
/// A chain `a/b/c` of folders holding one folder each is merged into one row, up to this long.
const CHAIN_CAP: usize = 16;

/// One row of the tree: an entry with what is below it, or a run of empty folders.
struct Node {
    e: Option<Entry>,
    /// Entries below it (loaded while the depth and node budgets last).
    kids: Vec<Node>,
    /// Its listing was cut at `READ_CAP`.
    unread: bool,
    /// A folder with nothing in it.
    empty: bool,
    /// More than one empty folder in a row, shown as one line (the count); the node has no entry.
    run: usize,
}

fn is_empty_dir(path: &Path) -> bool {
    fs::read_dir(path).map(|mut d| d.next().is_none()).unwrap_or(false)
}

/// The entries of `dir` as nodes, ordered folders with content, files, then empty folders
/// (several empty folders in a row become one `run` node). `levels` more levels are loaded.
fn load(dir: &Path, levels: usize, budget: &mut usize) -> (Vec<Node>, bool) {
    let mut all = entries(dir);
    let unread = all.len() > READ_CAP;
    all.truncate(READ_CAP);
    let (mut full, mut files, mut empties) = (Vec::new(), Vec::new(), Vec::new());
    for mut e in all {
        *budget = budget.saturating_sub(1);
        let mut node = Node { e: None, kids: Vec::new(), unread: false, empty: false, run: 0 };
        if e.is_dir {
            node.empty = is_empty_dir(&e.path);
            if !node.empty && levels > 0 && *budget > 0 {
                let (mut kids, mut cut) = load(&e.path, levels - 1, budget);
                // A folder holding only one folder reads as one row: `a/b/c`.
                for _ in 0..CHAIN_CAP {
                    let only_dir = kids.len() == 1 && kids[0].run == 0 && kids[0].e.as_ref().is_some_and(|k| k.is_dir) && !cut;
                    if !only_dir {
                        break;
                    }
                    let child = kids.remove(0);
                    let ce = child.e.as_ref().expect("checked above");
                    e.name = format!("{}/{}", e.name, ce.name);
                    node.empty = child.empty;
                    (kids, cut) = (child.kids, child.unread);
                }
                node.kids = kids;
                node.unread = cut;
            }
        }
        let is_dir = e.is_dir;
        node.e = Some(e);
        if is_dir && node.empty {
            empties.push(node);
        } else if is_dir {
            full.push(node);
        } else {
            files.push(node);
        }
    }
    let mut out = full;
    out.extend(files);
    match empties.len() {
        0 => {}
        1 => out.extend(empties),
        n => out.push(Node { e: None, kids: Vec::new(), unread: false, empty: false, run: n }),
    }
    (out, unread)
}

/// How many entries a node stands for in a `… N more` count.
fn weight(n: &Node) -> usize {
    if n.run > 0 { n.run } else { 1 }
}

struct Ctx<'a> {
    theme: &'a Theme,
    opts: &'a Options,
    git: &'a Option<(Repo, Status)>,
}

/// Flatten `nodes` into rows. With `cap`, each folder below the top level shows that
/// many entries and then a `… N more` row; folders are expanded down to `depth` levels.
fn flatten(cx: &Ctx, nodes: &[Node], unread: bool, lead: &str, level: usize, cap: Option<usize>, depth: usize, out: &mut Vec<Line<'static>>) {
    let c = &cx.theme.colors;
    let shown = match cap {
        Some(n) if level >= 2 && nodes.len() > n => &nodes[..n],
        _ => nodes,
    };
    let hidden: usize = nodes[shown.len()..].iter().map(weight).sum();
    let more = hidden > 0 || unread;
    for (i, n) in shown.iter().enumerate() {
        let last = i + 1 == shown.len() && !more;
        let prefix = format!("{lead}{}", if last { "└── " } else { "├── " });
        if n.run > 0 {
            out.push(Line::from(vec![
                Span::styled(prefix, Style::new().fg(c.muted.into())),
                Span::styled(format!("· {} empty folders", n.run), Style::new().fg(c.muted.into())),
            ]));
            continue;
        }
        let e = n.e.as_ref().expect("a node without a run has an entry");
        out.push(row(&prefix, e, cx.opts, state_of(e, cx.git), cx.theme, if n.empty { Some("(empty)") } else { None }));
        if !n.kids.is_empty() && level < depth {
            let next = format!("{lead}{}", if last { "    " } else { "│   " });
            flatten(cx, &n.kids, n.unread, &next, level + 1, cap, depth, out);
        }
    }
    if more {
        let text = match (unread, hidden) {
            (false, n) => format!("… {n} more"),
            (true, 0) => "… more not shown".to_string(),
            (true, n) => format!("… {n}+ more"),
        };
        out.push(Line::from(Span::styled(format!("{lead}└── {text}"), Style::new().fg(c.muted.into()).add_modifier(Modifier::BOLD))));
    }
}

pub fn render(path: &Path, theme: &Theme, opts: &Options) -> Result<Vec<Line<'static>>, Error> {
    let limit = opts.height.map(usize::from).unwrap_or(usize::MAX);
    let git = if opts.git { git::load(path) } else { None };
    let mut budget = NODE_CAP;
    let (tree, unread) = load(path, MAX_DEPTH - 1, &mut budget);
    let cx = Ctx { theme, opts, git: &git };
    let build = |cap: Option<usize>, depth: usize| {
        let mut out = Vec::new();
        flatten(&cx, &tree, unread, "", 1, cap, depth, &mut out);
        out
    };
    // Everything, if it fits.
    let full = build(None, MAX_DEPTH);
    if full.len() <= limit {
        return Ok(full);
    }
    // Else a few entries per folder, as deep as the room allows.
    for depth in (1..=MAX_DEPTH).rev() {
        let rows = build(Some(CAP), depth);
        if rows.len() <= limit {
            return Ok(rows);
        }
    }
    let mut rows = build(Some(CAP), 1);
    rows.truncate(limit);
    Ok(rows)
}

/// Git sign for an entry: a file's own state, or a dot for a folder with changes beneath it.
fn state_of(e: &Entry, ctx: &Option<(Repo, Status)>) -> Option<(char, Kind)> {
    let (repo, status) = ctx.as_ref()?;
    let rela = repo.relative(&e.path)?;
    if e.is_dir {
        status.dir(&rela).map(|k| (k.letter(), k))
    } else {
        git::sign(status.get(&rela)?)
    }
}
