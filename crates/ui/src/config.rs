//! The user's configuration of a tool: one TOML file shared by every tool and one of its own.
//!
//! ```toml
//! leader = "space"            # the key `leader` stands for in bindings
//! animations = "on"           # "on", "reduced" (spinner only) or "off"
//!
//! [keys]                      # action id -> key(s); an empty list unbinds the action
//! "nav.down" = ["j", "down"]
//! "change.stage" = "s"
//! "remote.push" = "leader r p"
//! ```
//!
//! `$XDG_CONFIG_HOME/ui/config.toml` (`$UI_CONFIG`) holds what all tools share; the tool's own
//! `$XDG_CONFIG_HOME/<tool>/config.toml` (`$UI_TOOL_CONFIG`) overrides it. A tool calls `init`
//! once at startup, before it builds its keymaps; `Keymap::new` then applies the result.
//! A file that is missing is fine; one that cannot be read or parsed is reported by `init` and
//! skipped, so a typo never leaves a tool without keys.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Config {
    pub leader: Option<String>,
    /// `on`, `reduced` or `off` (see `anim`).
    pub animations: Option<String>,
    /// Action id -> its keys.
    pub keys: BTreeMap<String, Vec<String>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Keys {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize, Default)]
struct File {
    leader: Option<String>,
    animations: Option<String>,
    #[serde(default)]
    keys: BTreeMap<String, Keys>,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

fn base() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

/// The two files read for `tool`, the shared one first.
pub fn paths(tool: &str) -> Vec<PathBuf> {
    let shared = std::env::var_os("UI_CONFIG").map(PathBuf::from).or_else(|| base().map(|b| b.join("ui/config.toml")));
    let own = std::env::var_os("UI_TOOL_CONFIG")
        .map(PathBuf::from)
        .or_else(|| base().map(|b| b.join(tool).join("config.toml")));
    shared.into_iter().chain(own).collect()
}

impl Config {
    /// Read `files` in order, later ones winning; the second value lists what was wrong.
    pub fn read(files: &[PathBuf]) -> (Config, Vec<String>) {
        let mut cfg = Config::default();
        let mut problems = Vec::new();
        for path in files {
            match Self::read_file(path) {
                Ok(Some(f)) => cfg.merge(f),
                Ok(None) => {}
                Err(e) => problems.push(e),
            }
        }
        (cfg, problems)
    }

    fn read_file(path: &Path) -> Result<Option<File>, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        toml::from_str::<File>(&text).map(Some).map_err(|e| format!("{}: {}", path.display(), e.message()))
    }

    fn merge(&mut self, f: File) {
        if f.leader.is_some() {
            self.leader = f.leader;
        }
        if f.animations.is_some() {
            self.animations = f.animations;
        }
        for (id, keys) in f.keys {
            self.keys.insert(
                id,
                match keys {
                    Keys::One(k) => vec![k],
                    Keys::Many(ks) => ks,
                },
            );
        }
    }
}

/// Load the configuration of `tool` for the rest of the process; what was wrong with the files
/// is returned for the tool to print before it takes over the screen.
pub fn init(tool: &str) -> Vec<String> {
    let (cfg, problems) = Config::read(&paths(tool));
    let _ = CONFIG.set(cfg);
    problems
}

/// The loaded configuration (empty before `init`).
pub fn get() -> &'static Config {
    CONFIG.get_or_init(Config::default)
}

#[cfg(test)]
mod tests;
