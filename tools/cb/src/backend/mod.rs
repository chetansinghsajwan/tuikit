//! Where the clipboard is read and written. There is no portable clipboard API, so each platform
//! gets a backend behind one trait, chosen at runtime from the environment. Backends call the
//! platform's own tools (`wl-clipboard`, `xclip`, `pbcopy`) rather than link a display library:
//! `cb` stays a small binary and the same code runs as a service and in a terminal.

use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// A place the clipboard text can be read from and written to.
pub trait Backend {
    /// The current clipboard text. `Ok(None)` when it is empty or holds something other than
    /// text; `Err` when the clipboard cannot be reached at all (no session, tool missing), so a
    /// recorder can tell "nothing copied" from "cannot see the clipboard".
    fn read(&self) -> Result<Option<String>, String>;

    /// Puts `text` on the clipboard.
    fn write(&self, text: &str) -> io::Result<()>;
}

/// A tool and its arguments.
type Tool = (&'static str, &'static [&'static str]);

/// A clipboard reached through a read command and a write command (`wl-paste`/`wl-copy`,
/// `xclip`, `pbpaste`/`pbcopy`). Environment variables the tools need are set on each call.
pub struct Tools {
    /// Prints the clipboard text on stdout.
    read: Tool,
    /// Takes the clipboard text on stdin.
    write: Tool,
    /// Variables added to the tools' environment.
    env: Vec<(&'static str, String)>,
}

impl Tools {
    /// Builds the command for a tool, with this backend's environment.
    fn command(&self, (program, args): Tool) -> Command {
        let mut cmd = Command::new(program);
        cmd.args(args).envs(self.env.iter().map(|(k, v)| (*k, v)));
        cmd
    }
}

impl Backend for Tools {
    fn read(&self) -> Result<Option<String>, String> {
        let out = self
            .command(self.read)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("{}: {e}", self.read.0))?;
        if !out.status.success() {
            // An empty clipboard also exits non-zero ("Nothing is copied"); only a failed
            // connection is an error.
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return if stderr.to_lowercase().contains("connect") {
                Err(format!("{}: {stderr}", self.read.0))
            } else {
                Ok(None)
            };
        }
        Ok(String::from_utf8(out.stdout).ok())
    }

    fn write(&self, text: &str) -> io::Result<()> {
        let mut child = self.command(self.write).stdin(Stdio::piped()).spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait()?;
        Ok(())
    }
}

/// The last resort: ask the terminal to set the clipboard (OSC 52). It can write but never read,
/// so it cannot record history.
pub struct Osc52;

impl Backend for Osc52 {
    fn read(&self) -> Result<Option<String>, String> {
        Err("no clipboard tool found (wl-clipboard, xclip or pbpaste); the terminal cannot be read"
            .into())
    }

    fn write(&self, text: &str) -> io::Result<()> {
        ui::clipboard::copy(text)
    }
}

/// The kinds of clipboard `cb` knows.
#[derive(Debug, PartialEq, Eq)]
pub enum Kind {
    /// macOS: `pbpaste` and `pbcopy`.
    MacOs,
    /// Wayland, including WSLg, with the display name the tools should connect to.
    Wayland(String),
    /// X11: `xclip`.
    X11,
    /// Nothing usable: only OSC 52 writes.
    Terminal,
}

/// What the environment offers, gathered in one place so the choice is a pure function.
pub struct Env {
    /// Running on macOS.
    pub macos: bool,
    /// `WAYLAND_DISPLAY`, when set and not empty.
    pub wayland_display: Option<String>,
    /// `DISPLAY` is set (an X11 session).
    pub x11: bool,
    /// A socket named `wayland-0` sits in the runtime directory.
    pub default_wayland_socket: bool,
}

impl Env {
    /// Reads the current process environment.
    pub fn current() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        let socket =
            var("XDG_RUNTIME_DIR").is_some_and(|dir| Path::new(&dir).join("wayland-0").exists());
        Self {
            macos: cfg!(target_os = "macos"),
            wayland_display: var("WAYLAND_DISPLAY"),
            x11: var("DISPLAY").is_some(),
            default_wayland_socket: socket,
        }
    }
}

/// Picks the clipboard kind. A Wayland session whose display variable is missing (a systemd user
/// service on WSLg does not inherit it) still counts when the default socket exists.
pub fn choose(env: &Env) -> Kind {
    if env.macos {
        Kind::MacOs
    } else if let Some(display) = &env.wayland_display {
        Kind::Wayland(display.clone())
    } else if env.default_wayland_socket {
        Kind::Wayland("wayland-0".to_string())
    } else if env.x11 {
        Kind::X11
    } else {
        Kind::Terminal
    }
}

/// The backend for a kind.
pub fn build(kind: Kind) -> Box<dyn Backend> {
    match kind {
        Kind::MacOs => {
            Box::new(Tools { read: ("pbpaste", &[]), write: ("pbcopy", &[]), env: Vec::new() })
        }
        Kind::Wayland(display) => Box::new(Tools {
            read: ("wl-paste", &["--type", "text", "--no-newline"]),
            write: ("wl-copy", &[]),
            env: vec![("WAYLAND_DISPLAY", display)],
        }),
        Kind::X11 => Box::new(Tools {
            read: ("xclip", &["-selection", "clipboard", "-out"]),
            write: ("xclip", &["-selection", "clipboard", "-in"]),
            env: Vec::new(),
        }),
        Kind::Terminal => Box::new(Osc52),
    }
}

/// The backend for this machine.
pub fn detect() -> Box<dyn Backend> {
    build(choose(&Env::current()))
}

#[cfg(test)]
mod tests;
