/// One action as the palette and the help list it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: &'static str,
    pub name: std::borrow::Cow<'static, str>,
    /// The keys bound to it, as shown (`space v c`, `j / down`).
    pub keys: String,
    pub desc: std::borrow::Cow<'static, str>,
    pub basic: bool,
}

impl super::keymap::Keymap {
    /// Every action, in the order the tool declared them.
    pub fn rows(&self) -> Vec<Row> {
        self.actions
            .iter()
            .map(|a| Row {
                id: a.id,
                name: a.name.into(),
                keys: self.keys_of(a.id),
                desc: a.desc.into(),
                basic: a.basic,
            })
            .collect()
    }
}

/// Let the user pick one of `rows` by name with the shared picker (`pick`, which must be
/// on `PATH`): the command palette of `tool`. Basic rows (movement and the like) are left out, the
/// columns have labels and a rule separates the search line from the results. `None` when
/// cancelled or when `pick` is unavailable. Inside zellij `pick` is a floating pane over the
/// tool, so the caller keeps its screen; elsewhere it gives the terminal to `pick` first.
pub fn choose(tool: &str, rows: &[Row], title: &str) -> Option<&'static str> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    // The commands run last come first (like VS Code and Zed), the rest in the tool's order.
    let rows: Vec<&Row> = crate::recent::order(
        rows.iter().filter(|r| !r.basic).collect(),
        &crate::recent::load(tool),
        |r| r.id,
    );
    let name_w =
        rows.iter().map(|r| r.name.chars().count()).max().unwrap_or(0).max("Command".len());
    let keys_w =
        rows.iter().map(|r| r.keys.chars().count()).max().unwrap_or(0).min(24).max("Keys".len());
    let mut input = format!("{:<name_w$}  {:<keys_w$}  Description\n", "Command", "Keys");
    for r in &rows {
        input
            .push_str(&format!("{:<name_w$}  {:<keys_w$}  {}\t{}\n", r.name, r.keys, r.desc, r.id));
    }
    let mut child = Command::new("pick")
        .args([
            "--no-preview",
            "--flat",
            "--header-lines",
            "1",
            "--title",
            title,
            "--delimiter",
            "\t",
            "--with-nth",
            "1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(input.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let id = text.lines().next()?.rsplit('\t').next()?;
    let picked = rows.iter().find(|r| r.id == id).map(|r| r.id)?;
    crate::recent::record(tool, picked);
    Some(picked)
}
