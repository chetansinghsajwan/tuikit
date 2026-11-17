//! Icons (Nerd Font glyphs) and their colors, all behind `IconProvider`: files and folders (the set
//! yazi ships), pane titles, git objects and symbol kinds. The `Theme` owns one, so tools ask
//! `theme.icons`. Data in `data.rs` is generated from yazi's preset theme (MIT).

mod data;

use data::{
    BLUETOOTH, BRANCH, COMMIT, DIAGNOSTICS, DIR, DIR_OPEN, DIRS, EXEC, EXTS, FILE, FILES, GIT, STASH,
    SUBMODULE, WIFI,
};
use crate::symbol::Kind;
use ratatui::style::Color;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icon {
    pub glyph: &'static str,
    pub color: Color,
}

const fn rgb(c: u32) -> Color {
    Color::Rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

fn find(table: &[(&'static str, &'static str, u32)], key: &str) -> Option<Icon> {
    let i = table.binary_search_by(|(n, _, _)| (*n).cmp(key)).ok()?;
    let (_, glyph, color) = table[i];
    Some(Icon { glyph, color: rgb(color) })
}

/// Where every icon comes from, so a thing looks the same in every tool. Owned by the `Theme`.
#[derive(Debug, Clone, Copy)]
pub struct IconProvider {
    /// The Bluetooth rune, for the Bluetooth pane.
    pub bluetooth: Icon,
    /// A Wi-Fi symbol, for the Wi-Fi pane.
    pub wifi: Icon,
    /// A git branch, for branch rows.
    pub branch: Icon,
    /// A git commit, for commit rows.
    pub commit: Icon,
    /// An archive box, for stash rows.
    pub stash: Icon,
    /// A git box, for submodules (a folder that is a repository of its own).
    pub submodule: Icon,
    /// The git logo, for git panes (yazi's color for git files).
    pub git: Icon,
    /// A warning triangle, for the diagnostics pane.
    pub diagnostics: Icon,
}

impl Default for IconProvider {
    fn default() -> Self {
        IconProvider {
            bluetooth: BLUETOOTH,
            wifi: WIFI,
            branch: BRANCH,
            commit: COMMIT,
            stash: STASH,
            submodule: SUBMODULE,
            git: GIT,
            diagnostics: DIAGNOSTICS,
        }
    }
}

impl IconProvider {
    /// The glyph for a kind of code symbol; its color is the kind's own (`Kind::color`).
    pub fn symbol(&self, kind: Kind) -> &'static str {
        match kind {
            Kind::Module => "\u{f0169}",
            Kind::Class | Kind::Struct => "\u{f0831}",
            Kind::Interface => "\u{f0b6f}",
            Kind::Enum => "\u{f12a7}",
            Kind::Function | Kind::Method | Kind::Constructor => "\u{f0295}",
            Kind::Field => "\u{f0e69}",
            Kind::Variable => "\u{f0ae7}",
            Kind::Constant => "\u{f03ff}",
            Kind::TypeParameter => "\u{f0b68}",
            Kind::Other => "\u{f0a7a}",
        }
    }

    /// An icon for a well-known pane title (`Files`, `Shell History`, `Branches`, `Buffers`, ...), so
    /// pickers and panels look alike whatever tool opens them. Case does not matter.
    pub fn title(&self, title: &str) -> Option<Icon> {
        let t = title.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| t.contains(w));
        let icon = |glyph: &'static str, color: u32| Some(Icon { glyph, color: rgb(color) });
        if has(&["bluetooth"]) {
            Some(BLUETOOTH)
        } else if has(&["wi-fi", "wifi"]) {
            Some(WIFI)
        } else if has(&["propert"]) {
            icon("\u{f05a}", 0x59c2ff)
        } else if has(&["branch", "refs", "tags"]) {
            Some(BRANCH)
        } else if has(&["shell", "command history"]) {
            icon("\u{f120}", 0x89e051)
        } else if has(&["clipboard"]) {
            icon("\u{f0ea}", 0x6d8086)
        } else if has(&["history"]) {
            icon("\u{f1da}", 0xe8a838)
        } else if has(&["commit", "stash", "log", "changes", "changed", "status"]) {
            Some(GIT)
        } else if has(&["diagnostic", "problem"]) {
            Some(DIAGNOSTICS)
        } else if has(&["buffer"]) {
            icon("\u{f0c5}", 0x6d8086)
        } else if has(&["jump"]) {
            icon("\u{f0e2}", 0x6d8086)
        } else if has(&["symbol"]) {
            icon("\u{f1b2}", 0xa074c4)
        } else if has(&["theme"]) {
            icon("\u{f1fc}", 0xf245fb)
        } else if has(&["action"]) {
            icon("\u{f0eb}", 0xffc107)
        } else if has(&["command"]) {
            icon("\u{f120}", 0x89e051)
        } else if has(&["register"]) {
            icon("\u{f0ea}", 0x6d8086)
        } else if has(&["file", "find", "open"]) {
            Some(DIR_OPEN)
        } else if has(&["grep", "search", "pick"]) {
            icon("\u{f002}", 0x59c2ff)
        } else {
            None
        }
    }

    /// Icon for a folder; `open` picks the open-folder glyph (expanded or hovered).
    pub fn dir(&self, name: &str, open: bool) -> Icon {
        match find(DIRS, &name.to_lowercase()) {
            Some(i) => i,
            None if open => DIR_OPEN,
            None => DIR,
        }
    }

    /// Icon for a file by exact name, then extension; executables fall back to the
    /// executable glyph.
    pub fn file(&self, name: &str, executable: bool) -> Icon {
        let lower = name.to_lowercase();
        if let Some(i) = find(FILES, &lower) {
            return i;
        }
        if let Some(ext) = Path::new(&lower).extension().and_then(|e| e.to_str())
            && let Some(i) = find(EXTS, ext)
        {
            return i;
        }
        if executable { EXEC } else { FILE }
    }
}

#[cfg(test)]
mod tests;
