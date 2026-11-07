//! Directory tree, drawn with the common file row (`ui::tree::file_row`): everything when it fits (down to four levels), else two entries
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

struct Entry {
    name: String,
    is_dir: bool,
    mode: u32,
    mtime: Option<SystemTime>,
    path: std::path::PathBuf,
}

/// The entries of `dir`: dot files too, but not what git ignores, and never `.git`.
fn entries(dir: &Path) -> Vec<Entry> {
    let walk = ignore::WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    let mut v: Vec<Entry> = walk
        .flatten()
        .filter(|e| e.depth() == 1)
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
                path: e.into_path(),
            })
        })
        .collect();
    v.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
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
            (
                ch,
                match ch {
                    'r' => c.success,
                    'w' => c.warning,
                    _ => c.error,
                },
            )
        };
        out.push(Span::styled(text.to_string(), Style::new().fg(color.into())));
        if i == 2 || i == 5 {
            out.push(Span::raw(" "));
        }
    }
    out
}

/// One entry as the common file-tree row (`ui::tree::file_row`): guides, icon, git-colored name, and with a
/// width the right column (permissions, age) and the git sign. The name shrinks to leave them room.
fn row(
    depth: usize,
    open: bool,
    e: &Entry,
    opts: &Options,
    state: Option<(char, Kind)>,
    theme: &Theme,
    note: Option<&str>,
) -> Line<'static> {
    let c = &theme.colors;
    let (mut trailing, mut signs) = (Vec::new(), Vec::new());
    if opts.width.is_some() {
        trailing = perm_spans(e.mode, theme);
        trailing.push(Span::styled(
            format!("  {:>8}", relative(e.mtime)),
            Style::new().fg(c.muted.into()),
        ));
        if opts.git {
            trailing.push(Span::raw(" "));
            signs =
                state.map(|(ch, kind)| (ch, git::kind_color(theme, kind))).into_iter().collect();
        }
    }
    let row = ui::tree::FileRow {
        depth,
        name: &e.name,
        is_dir: e.is_dir,
        open,
        executable: e.mode & 0o111 != 0,
        tint: state.map(|(_, kind)| git::kind_color(theme, kind)),
        focused: true,
        note,
        trailing,
        signs,
        ..Default::default()
    };
    ui::tree::file_row_line(theme, &row, opts.width)
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
                    let only_dir = kids.len() == 1
                        && kids[0].run == 0
                        && kids[0].e.as_ref().is_some_and(|k| k.is_dir)
                        && !cut;
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
fn flatten(
    cx: &Ctx,
    nodes: &[Node],
    unread: bool,
    level: usize,
    cap: Option<usize>,
    depth: usize,
    out: &mut Vec<Line<'static>>,
) {
    let c = &cx.theme.colors;
    let shown = match cap {
        Some(n) if level >= 2 && nodes.len() > n => &nodes[..n],
        _ => nodes,
    };
    let hidden: usize = nodes[shown.len()..].iter().map(weight).sum();
    let more = hidden > 0 || unread;
    // Remarks (`… 3 more`) start under the names, after the guides and the icon column.
    let remark = |text: String, style: Style| {
        Line::from(vec![
            ui::tree::guides(cx.theme, level - 1),
            Span::styled(format!("  {text}"), style),
        ])
    };
    for n in shown {
        if n.run > 0 {
            out.push(remark(format!("· {} empty folders", n.run), Style::new().fg(c.muted.into())));
            continue;
        }
        let e = n.e.as_ref().expect("a node without a run has an entry");
        let open = !n.kids.is_empty() && level < depth;
        out.push(row(
            level - 1,
            open,
            e,
            cx.opts,
            state_of(e, cx.git),
            cx.theme,
            if n.empty { Some("(empty)") } else { None },
        ));
        if open {
            flatten(cx, &n.kids, n.unread, level + 1, cap, depth, out);
        }
    }
    if more {
        let text = match (unread, hidden) {
            (false, n) => format!("… {n} more"),
            (true, 0) => "… more not shown".to_string(),
            (true, n) => format!("… {n}+ more"),
        };
        out.push(remark(text, Style::new().fg(c.muted.into()).add_modifier(Modifier::BOLD)));
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
        flatten(&cx, &tree, unread, 1, cap, depth, &mut out);
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
