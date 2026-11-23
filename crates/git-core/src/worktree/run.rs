//! Running the `git` command line.

use super::*;

impl Repo {
    /// Run `git` in the work tree (never prompting, never opening an editor) and return its stdout,
    /// or the last line of its error as an `Error`.
    pub(crate) fn git(&self, args: &[&str], paths: &[PathBuf]) -> Result<String, Error> {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.root).args(["--no-pager", "-c", "core.quotepath=off"]).args(args);
        if !paths.is_empty() {
            cmd.arg("--").args(paths);
        }
        let out =
            cmd.env("GIT_TERMINAL_PROMPT", "0").env("GIT_EDITOR", "true").output().map_err(err)?;
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
        }
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let msg = if msg.is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        } else {
            msg
        };
        Err(Error(msg.lines().last().unwrap_or("git failed").to_string()))
    }

    /// Like `git`, for commands that talk to a remote: never prompts (the TUI owns the
    /// terminal), and ssh runs in batch mode so a passphrase prompt cannot take over the screen
    /// (keys in an agent still work).
    pub(crate) fn git_net(&self, args: &[&str]) -> Result<String, Error> {
        let base = std::env::var("GIT_SSH_COMMAND").ok().or_else(|| {
            let out = Command::new("git")
                .current_dir(&self.root)
                .args(["config", "--get", "core.sshCommand"])
                .output()
                .ok()?;
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            (!s.is_empty()).then_some(s)
        });
        let ssh = format!("{} -o BatchMode=yes", base.unwrap_or_else(|| "ssh".into()));
        let out = Command::new("git")
            .current_dir(&self.root)
            .args(["--no-pager"])
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_EDITOR", "true")
            .env("GIT_SSH_COMMAND", ssh)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(err)?;
        if out.status.success() {
            return Ok(format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(Error(msg.lines().last().unwrap_or("git failed").to_string()))
    }
}
