//! Content search with ripgrep's own crates (`grep`, `ignore`). The query is
//! the pattern; every query change cancels the running search and starts a new
//! one, streaming matches into the picker.

use crate::candidate::{Candidate, Loc};
use grep::matcher::Matcher;
use grep::regex::RegexMatcherBuilder;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, SearcherBuilder};
use nucleo::Injector;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// Stop after this many matches so a loose pattern cannot flood the picker.
const MAX_MATCHES: usize = 20_000;
/// Longest line kept per match, in bytes.
const MAX_LINE: usize = 400;

/// What narrows a search, like the toggles and the file filters of the search panels of VS Code and Zed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Match the case exactly (off: smart case, which ignores case until the query has a capital).
    pub case_sensitive: bool,
    /// Match whole words only.
    pub whole_word: bool,
    /// The query is a regular expression (off: it is plain text).
    pub regex: bool,
    /// Only files matching these globs, separated by commas (`*.rs, src/`).
    pub include: String,
    /// Not the files matching these globs (`target/, *.lock`).
    pub exclude: String,
}

impl Default for Options {
    fn default() -> Self {
        Options { case_sensitive: false, whole_word: false, regex: true, include: String::new(), exclude: String::new() }
    }
}

/// The globs of `opts` as an override set for the walker: includes whitelist, excludes start with `!`.
/// `None` when there are none (or one does not parse: the search then is not narrowed).
fn globs(root: &std::path::Path, opts: &Options) -> Option<ignore::overrides::Override> {
    let split = |s: &str| s.split(',').map(str::trim).filter(|p| !p.is_empty()).map(str::to_string).collect::<Vec<_>>();
    let (inc, exc) = (split(&opts.include), split(&opts.exclude));
    if inc.is_empty() && exc.is_empty() {
        return None;
    }
    let mut b = ignore::overrides::OverrideBuilder::new(root);
    for p in inc {
        b.add(&p).ok()?;
    }
    for p in exc {
        b.add(&format!("!{p}")).ok()?;
    }
    b.build().ok()
}

/// Search `root` for `query` on a background thread. Set `cancel` to stop it.
/// An invalid regex (likely mid-typing) falls back to a literal search.
pub fn spawn(root: PathBuf, query: String, opts: Options, inj: Injector<Candidate>, cancel: Arc<AtomicBool>) {
    if query.is_empty() {
        return;
    }
    thread::spawn(move || {
        let build = |fixed: bool| RegexMatcherBuilder::new().case_smart(!opts.case_sensitive).word(opts.whole_word).fixed_strings(fixed).build(&query);
        let Ok(matcher) = (if opts.regex { build(false).or_else(|_| build(true)) } else { build(true) }) else { return };
        let mut searcher = SearcherBuilder::new().binary_detection(BinaryDetection::quit(0)).line_number(true).build();

        let mut found = 0usize;
        let mut walk = ignore::WalkBuilder::new(&root);
        walk.hidden(false).filter_entry(|e| e.file_name() != ".git");
        if let Some(ov) = globs(&root, &opts) {
            walk.overrides(ov);
        }
        for entry in walk.build().flatten() {
            if cancel.load(Ordering::Relaxed) || found >= MAX_MATCHES {
                return;
            }
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            let Some(rela) = path.strip_prefix(&root).unwrap_or(path).to_str().map(str::to_string) else { continue };
            let _ = searcher.search_path(
                &matcher,
                path,
                UTF8(|lnum, line| {
                    if cancel.load(Ordering::Relaxed) || found >= MAX_MATCHES {
                        return Ok(false);
                    }
                    let m = matcher.find(line.as_bytes()).ok().flatten();
                    let text = clip(line.trim_end_matches(['\n', '\r']));
                    let (start, end) = m.map(|m| (m.start(), m.end())).unwrap_or((0, 0));
                    // Keep the hit inside the clipped text.
                    let (start, end) = (start.min(text.len()), end.min(text.len()));
                    let col = line[..start.min(line.len())].chars().count() as u32 + 1;
                    let hit = if text.is_char_boundary(start) && text.is_char_boundary(end) { start..end } else { 0..0 };
                    let c = Candidate::grep(Loc { path: rela.clone(), line: lnum as u32, col, text, hit });
                    inj.push(c, |c, cols| cols[0] = c.display.as_str().into());
                    found += 1;
                    Ok(true)
                }),
            );
        }
    });
}

fn clip(s: &str) -> String {
    if s.len() <= MAX_LINE {
        return s.to_string();
    }
    let mut end = MAX_LINE;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

#[cfg(test)]
mod tests;
