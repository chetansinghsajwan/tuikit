//! Icons (Nerd Font glyphs) and their colors, all behind `IconProvider`: files and folders (the set
//! yazi ships), pane titles, git objects and symbol kinds. The `Theme` owns one, so tools ask
//! `theme.icons`. Data in `data.rs` is generated from yazi's preset theme (MIT).

mod data;

use crate::symbol::Kind;
use data::{
    BLUETOOTH, BRANCH, COMMIT, DIAGNOSTICS, DIR, DIR_OPEN, DIRS, EXEC, EXTS, FILE, FILES, GIT,
    STASH, SUBMODULE, WIFI,
};
use ratatui::style::{Color, Style};
use std::path::Path;

/// A glyph and its optional color: the unit every icon lookup returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icon {
    pub glyph: &'static str,
    /// `None` for a glyph that takes the colour of the style it is drawn in (guides, folds, rules).
    pub color: Option<Color>,
}

impl Icon {
    /// The style to draw it in: its own colour, or none (so the surrounding style shows through).
    pub fn style(self) -> Style {
        match self.color {
            Some(c) => Style::new().fg(c),
            None => Style::new(),
        }
    }
}

/// Turn a `0xRRGGBB` number into a truecolor value; `const` so the tables can use it.
const fn rgb(c: u32) -> Color {
    Color::Rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// A glyph with no colour of its own: it takes the colour of the style it is drawn in.
const fn plain(glyph: &'static str) -> Icon {
    Icon { glyph, color: None }
}

/// Look up `key` in one of the sorted icon tables by binary search.
fn find(table: &[(&'static str, &'static str, u32)], key: &str) -> Option<Icon> {
    let i = table.binary_search_by(|(n, _, _)| (*n).cmp(key)).ok()?;
    let (_, glyph, color) = table[i];
    Some(Icon { glyph, color: Some(rgb(color)) })
}

/// Where every icon comes from, so a thing looks the same in every tool. Owned by the `Theme`.
///
/// Every field and method hands out an `Icon` (a glyph and an optional colour; `None` takes the colour
/// of the style it is drawn in), never a bare string or char. Glyphs are written as literal characters,
/// not `\u{...}` codes.
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
    /// The arrow of a folded row (with the space after it).
    pub fold_closed: Icon,
    /// The arrow of an unfolded row (with the space after it).
    pub fold_open: Icon,
    /// The vertical line of tree guides and table borders.
    pub guide: Icon,
    /// A marked row of a list or table.
    pub mark: Icon,
    /// The horizontal rule.
    pub divider: Icon,
    /// The left and right ends of a horizontal rule that cuts across a frame.
    pub divider_left: Icon,
    pub divider_right: Icon,
    /// The rounded caps of a pill (Powerline glyphs, so they need a Nerd Font like the icons).
    pub pill_left: Icon,
    pub pill_right: Icon,
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
            fold_closed: plain("▸ "),
            fold_open: plain("▾ "),
            guide: plain("│"),
            mark: plain("●"),
            divider: plain("─"),
            divider_left: plain("├"),
            divider_right: plain("┤"),
            pill_left: plain(""),
            pill_right: plain(""),
        }
    }
}

impl IconProvider {
    /// The icon for a kind of code symbol. It has no colour of its own: `Kind::color` gives it.
    pub fn symbol(&self, kind: Kind) -> Icon {
        plain(match kind {
            Kind::Module => "󰅩",
            Kind::Class | Kind::Struct => "󰠱",
            Kind::Interface => "󰭯",
            Kind::Enum => "󱊧",
            Kind::Function | Kind::Method | Kind::Constructor => "󰊕",
            Kind::Field => "󰹩",
            Kind::Variable => "󰫧",
            Kind::Constant => "󰏿",
            Kind::TypeParameter => "󰭨",
            Kind::Other => "󰩺",
        })
    }

    /// An icon for a well-known pane title (`Files`, `Shell History`, `Branches`, `Buffers`, ...), so
    /// pickers and panels look alike whatever tool opens them. Case does not matter.
    pub fn title(&self, title: &str) -> Option<Icon> {
        let t = title.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| t.contains(w));
        let icon = |glyph: &'static str, color: u32| Some(Icon { glyph, color: Some(rgb(color)) });
        if has(&["bluetooth"]) {
            Some(BLUETOOTH)
        } else if has(&["wi-fi", "wifi"]) {
            Some(WIFI)
        } else if has(&["propert"]) {
            icon("", 0x59c2ff)
        } else if has(&["branch", "refs", "tags"]) {
            Some(BRANCH)
        } else if has(&["shell", "command history"]) {
            icon("", 0x89e051)
        } else if has(&["clipboard"]) {
            icon("", 0x6d8086)
        } else if has(&["history"]) {
            icon("", 0xe8a838)
        } else if has(&["commit", "stash", "log", "changes", "changed", "status"]) {
            Some(GIT)
        } else if has(&["diagnostic", "problem"]) {
            Some(DIAGNOSTICS)
        } else if has(&["buffer"]) {
            icon("", 0x6d8086)
        } else if has(&["jump"]) {
            icon("", 0x6d8086)
        } else if has(&["symbol"]) {
            icon("", 0xa074c4)
        } else if has(&["theme"]) {
            icon("", 0xf245fb)
        } else if has(&["action"]) {
            icon("", 0xffc107)
        } else if has(&["command"]) {
            icon("", 0x89e051)
        } else if has(&["register"]) {
            icon("", 0x6d8086)
        } else if has(&["file", "find", "open"]) {
            Some(DIR_OPEN)
        } else if has(&["grep", "search", "pick"]) {
            icon("", 0x59c2ff)
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
