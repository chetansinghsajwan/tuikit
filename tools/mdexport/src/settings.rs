//! What a PDF looks like: paper, margins, fonts, size. The defaults come from
//! `$XDG_CONFIG_HOME/mdexport/config.toml` (Nix writes it with the stylix fonts), the command line and the
//! helix popup override them.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// `a4` or `letter`.
    pub paper: String,
    pub margin_cm: f32,
    /// The font of the text and of code (family names; the embedded ones are the fallback).
    pub font: String,
    pub mono: String,
    pub size_pt: f32,
    pub justify: bool,
    pub page_numbers: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { paper: "a4".into(), margin_cm: 2.2, font: "Libertinus Serif".into(), mono: "DejaVu Sans Mono".into(), size_pt: 11.0, justify: true, page_numbers: true }
    }
}

impl Settings {
    pub fn path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("mdexport/config.toml"))
    }

    /// The defaults, then the config file (a missing or broken file changes nothing).
    pub fn load() -> Settings {
        Self::path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// `key=value` lines, for tools that show the defaults (the helix popup).
    pub fn lines(&self) -> String {
        format!("paper={}\nmargin={}\nfont={}\nmono={}\nsize={}\njustify={}\nnumbers={}\n", self.paper, self.margin_cm, self.font, self.mono, self.size_pt, self.justify, self.page_numbers)
    }
}

#[cfg(test)]
mod tests;
