//! What the picker lists. Plain candidates are just text; grep results also
//! carry where the match is, and git items what they point at, for the
//! preview and the output.

use std::ops::Range;

/// One line of the picker: what it shows and matches, plus where it points, if anywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Text shown in the list and matched against the query.
    pub display: String,
    pub loc: Option<Loc>,
    pub git: Option<GitItem>,
    pub buffer: Option<Buffer>,
    /// The whole input line when `display` shows only part of it (`--with-nth`);
    /// that line is what accepting prints and what a preview command receives.
    pub raw: Option<String>,
    /// When it happened (seconds since the epoch), for input that carries times (`--timestamps`).
    pub time: Option<i64>,
    /// Where it sits in a hierarchy, for lists of things inside other things (symbols).
    pub hier: Option<Hier>,
}

/// Where a grep match is in the files, for the preview and the output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loc {
    /// Path relative to the search root.
    pub path: String,
    /// 1-based line and column of the match.
    pub line: u32,
    pub col: u32,
    /// The matching line (trimmed) and the match's byte range within it.
    pub text: String,
    pub hit: Range<usize>,
}

/// The kind of git thing a candidate is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitKind {
    Branch,
    Remote,
    Tag,
    Commit,
    Stash,
}

/// A ref, commit or stash entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitItem {
    pub kind: GitKind,
    /// Short name (`main`, `v1`), short hash, or `stash@{0}`.
    pub name: String,
    /// What to preview and print: the full commit hash.
    pub oid: String,
    pub subject: String,
    /// Commit time, seconds since the epoch.
    pub time: i64,
    /// The checked-out branch.
    pub is_head: bool,
}

/// An open editor buffer. The host (an editor embedding `pick`) fills it in;
/// `display` is the buffer's name and is what the query matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buffer {
    /// What accepting returns: an id the host maps back to the buffer.
    pub value: String,
    /// Path relative to the picker root, for the git state; `None` for scratch buffers.
    pub path: Option<String>,
    /// Flags shown before the name, e.g. `+` modified, `*` current.
    pub flags: String,
    /// 1-based line to bring into view in the preview (the cursor line).
    pub focus: Option<u32>,
    /// The buffer's text as shown in the editor (unsaved changes included).
    pub text: std::sync::Arc<str>,
}

impl Candidate {
    /// A candidate that is just text.
    pub fn plain(s: impl Into<String>) -> Self {
        Candidate {
            display: s.into(),
            loc: None,
            git: None,
            buffer: None,
            raw: None,
            time: None,
            hier: None,
        }
    }

    /// A grep match, shown as `path:line: text`.
    pub fn grep(loc: Loc) -> Self {
        Candidate {
            display: format!("{}:{}: {}", loc.path, loc.line, loc.text),
            loc: Some(loc),
            git: None,
            buffer: None,
            raw: None,
            time: None,
            hier: None,
        }
    }

    /// Matched against the query as `name subject`, so either finds it.
    pub fn git_item(item: GitItem) -> Self {
        Candidate {
            display: format!("{} {}", item.name, item.subject),
            loc: None,
            git: Some(item),
            buffer: None,
            raw: None,
            time: None,
            hier: None,
        }
    }

    /// Show `display` but keep the whole `line` for output and previews.
    pub fn with_raw(display: impl Into<String>, line: impl Into<String>) -> Self {
        Candidate {
            display: display.into(),
            loc: None,
            git: None,
            buffer: None,
            raw: Some(line.into()),
            time: None,
            hier: None,
        }
    }

    /// An open editor buffer, shown as `display`.
    pub fn buffer(display: impl Into<String>, buffer: Buffer) -> Self {
        Candidate {
            display: display.into(),
            loc: None,
            git: None,
            buffer: Some(buffer),
            raw: None,
            time: None,
            hier: None,
        }
    }

    /// The path the preview and git state refer to (file candidates only).
    pub fn path(&self) -> &str {
        if let Some(b) = &self.buffer {
            return b.path.as_deref().unwrap_or(&self.display);
        }
        self.loc.as_ref().map(|l| l.path.as_str()).unwrap_or(&self.display)
    }

    /// What is printed on accept: `path:line:col:text` for matches (ripgrep's
    /// `--vimgrep` format); the name for refs and stashes, the full hash for
    /// commits; the text itself otherwise.
    pub fn output(&self) -> String {
        if let Some(b) = &self.buffer {
            return b.value.clone();
        }
        if let Some(raw) = &self.raw {
            return raw.clone();
        }
        if let Some(l) = &self.loc {
            return format!("{}:{}:{}:{}", l.path, l.line, l.col, l.text);
        }
        match &self.git {
            Some(g) if g.kind == GitKind::Commit => g.oid.clone(),
            Some(g) => g.name.clone(),
            None => self.display.clone(),
        }
    }
}

/// What a candidate sits inside, for the hierarchy view: a symbol under its struct, under its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hier {
    /// Identifies this candidate among the others, so ancestors shared by several candidates merge.
    pub id: u32,
    /// `None` draws the candidate with no symbol icon.
    pub kind: Option<ui::symbol::Kind>,
    /// Orders siblings when nothing is typed (the line in the file).
    pub order: u32,
    /// The things above it, outermost first. One that is not a candidate itself is drawn as a header row.
    pub ancestors: Vec<Ancestor>,
}

/// A thing that holds a candidate in the hierarchy view: a file or a symbol above it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ancestor {
    pub id: u32,
    /// `None` is a file: its label is a path and it takes a file icon.
    pub kind: Option<ui::symbol::Kind>,
    pub label: String,
    pub order: u32,
}

impl Candidate {
    /// A candidate drawn in the hierarchy view; `display` is what the query matches.
    pub fn hier(display: impl Into<String>, raw: impl Into<String>, hier: Hier) -> Self {
        Candidate { hier: Some(hier), ..Candidate::with_raw(display, raw) }
    }
}
