//! Recently used commands, for the command palettes: the last ones run come first, like in
//! VS Code and Zed. One small file (`$XDG_STATE_HOME/ui/recent`, default
//! `~/.local/state/ui/recent`), one `tool<TAB>id` line per entry, the newest last, kept short per
//! tool. Failing to read or write it never stops a tool: the palette just has no history.

use std::fs;
use std::path::{Path, PathBuf};

/// How many commands are remembered per tool.
const KEPT: usize = 20;

/// Where the history lives (`UI_RECENT_FILE` overrides it, for tests).
pub fn path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("UI_RECENT_FILE") {
        return Some(PathBuf::from(p));
    }
    let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(state.join("ui/recent"))
}

fn read(path: &Path) -> Vec<(String, String)> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| l.split_once('\t')).map(|(t, i)| (t.to_string(), i.to_string())).collect()
}

/// The ids last run in `tool`, newest first.
pub fn load(tool: &str) -> Vec<String> {
    path().map(|p| load_from(&p, tool)).unwrap_or_default()
}

/// Remember that `id` was just run in `tool`.
pub fn record(tool: &str, id: &str) {
    if let Some(p) = path() {
        record_to(&p, tool, id);
    }
}

pub fn load_from(path: &Path, tool: &str) -> Vec<String> {
    read(path).into_iter().rev().filter(|(t, _)| t == tool).map(|(_, i)| i).collect()
}

pub fn record_to(path: &Path, tool: &str, id: &str) {
    let mut entries = read(path);
    entries.retain(|(t, i)| !(t == tool && i == id));
    entries.push((tool.to_string(), id.to_string()));
    // Keep the newest few of this tool; other tools' lines stay as they are.
    let mut seen = 0;
    let mut keep: Vec<(String, String)> = Vec::new();
    for e in entries.into_iter().rev() {
        if e.0 == tool {
            seen += 1;
            if seen > KEPT {
                continue;
            }
        }
        keep.push(e);
    }
    keep.reverse();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let text: String = keep.iter().map(|(t, i)| format!("{t}\t{i}\n")).collect();
    let _ = fs::write(path, text);
}

/// `items` with the recently used ones first (newest first), the rest in their order.
pub fn order<T>(items: Vec<T>, recent: &[String], id_of: impl Fn(&T) -> &str) -> Vec<T> {
    let mut first: Vec<Option<T>> = (0..recent.len()).map(|_| None).collect();
    let mut rest = Vec::new();
    for item in items {
        match recent.iter().position(|r| r == id_of(&item)) {
            Some(i) => first[i] = Some(item),
            None => rest.push(item),
        }
    }
    first.into_iter().flatten().chain(rest).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ui-recent-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d.join("state/recent")
    }

    #[test]
    fn the_newest_comes_first_repeats_move_up_and_tools_are_separate() {
        let f = file("order");
        assert!(load_from(&f, "files").is_empty());
        record_to(&f, "files", "a");
        record_to(&f, "files", "b");
        record_to(&f, "git", "x");
        record_to(&f, "files", "a");
        assert_eq!(load_from(&f, "files"), vec!["a", "b"]);
        assert_eq!(load_from(&f, "git"), vec!["x"]);
    }

    #[test]
    fn only_the_newest_few_per_tool_are_kept() {
        let f = file("cap");
        record_to(&f, "other", "keep");
        for i in 0..30 {
            record_to(&f, "files", &format!("id{i}"));
        }
        let got = load_from(&f, "files");
        assert_eq!(got.len(), KEPT);
        assert_eq!((got[0].as_str(), got[KEPT - 1].as_str()), ("id29", "id10"));
        assert_eq!(load_from(&f, "other"), vec!["keep"]);
    }

    #[test]
    fn order_puts_recents_first_and_ignores_unknown_ids() {
        let items = vec!["new", "rename", "delete", "paste"];
        let recent = vec!["paste".to_string(), "gone".to_string(), "new".to_string()];
        assert_eq!(order(items, &recent, |s| s), vec!["paste", "new", "rename", "delete"]);
    }

    #[test]
    fn a_missing_or_garbled_file_means_no_history() {
        let f = file("garbled");
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(&f, "no tab here\n\tx\nfiles\tok\n").unwrap();
        assert_eq!(load_from(&f, "files"), vec!["ok"]);
    }
}
