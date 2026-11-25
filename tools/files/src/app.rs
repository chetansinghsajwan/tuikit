//! The standalone explorer: tabs of the shared `Panel`, a preview and properties pane, and the
//! //! hand-offs to the editor, `pick` and the system opener.

use files::meta::{self, DirSize, Meta};
use files::{Outcome, Panel, PanelOptions, Search, ops};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use ui::Pane;
use ui::app::{Host, Navigable, Request, Tabbed, with_overlay, with_suspended};

/// What the command line asked for, passed from `main` so this module does not parse arguments.
pub struct Options {
    pub dir: PathBuf,
    pub chooser_file: Option<PathBuf>,
    pub cwd_file: Option<PathBuf>,
    pub hidden: bool,
    pub preview: bool,
    pub git: bool,
}

/// The last preview, kept until the cursor, the pane size or the file changes.
struct Preview {
    key: (PathBuf, u16, u16, bool),
    lines: Result<Vec<Line<'static>>, String>,
}

/// The state of the running explorer: its tabs, the preview cache and the properties pane.
struct Explorer {
    opts: Options,
    tabs: ui::tabs::Tabs<Panel>,
    preview: Option<Preview>,
    scroll: ui::scroll::PreviewScroll,
    show_meta: bool,
    meta: Option<(PathBuf, Option<Meta>)>,
    dirsize: Option<DirSize>,
    /// The folder-size count last drawn, to see when it moved.
    shown: Option<(u64, bool)>,
}

/// Runs the explorer on the shared app shell. On exit it writes the folder it ended in to
/// `--cwd-file`, so a shell can follow.
pub fn run(opts: Options) -> io::Result<()> {
    let dir = opts.dir.canonicalize()?;
    let panel_opts = panel_opts(&opts);
    let mut explorer = Explorer {
        tabs: ui::tabs::Tabs::new(Panel::new(dir, panel_opts)),
        preview: None,
        scroll: Default::default(),
        show_meta: true,
        meta: None,
        dirsize: None,
        shown: None,
        opts,
    };
    let mut commands = files::keys::hosted_commands();
    commands.add(crate::commands::ACTIONS, crate::binds::DEFAULT_BINDINGS);
    let result = ui::app::Shell::new("files")
        .commands(commands)
        .groups(files::keys::GROUPS)
        .tick_interval(Duration::from_millis(100))
        .run(&mut explorer);
    if let Some(file) = &explorer.opts.cwd_file {
        fs::write(file, explorer.tabs.active().root().as_os_str().as_encoded_bytes())?;
    }
    result
}

impl ui::app::Screen for Explorer {
    fn render(&mut self, cx: &ui::Context, area: Rect, buf: &mut Buffer) {
        self.draw(area, buf, cx);
        self.shown = self.dirsize.as_ref().map(DirSize::state);
    }

    /// A prompt, a question or a bookmark wait is open, or a paste runs and the key cancels it.
    fn captures_input(&self, ev: &Event) -> bool {
        self.tabs.active().captures_input(ev)
    }

    /// The panel's own keys while it captures them.
    fn on_event(&mut self, ev: &Event, host: &mut dyn Host) -> Request {
        if self.tabs.active().captures_input(ev) {
            let action = self.panel().handle_captured(ev);
            return self.carry_out(action, host);
        }
        Request::None
    }

    /// The panel's actions run on the panel, the app's own (tabs, properties) here.
    fn run_action(&mut self, id: &'static str, host: &mut dyn Host) -> Request {
        if files::keys::is_own_action(id) {
            let action = self.panel().run(id);
            self.carry_out(action, host)
        } else {
            self.command(id);
            Request::None
        }
    }

    fn on_keypress(&mut self) {
        self.panel().clear_message();
    }

    fn on_unbound_keys(&mut self, keys: &str) {
        self.panel().notify(format!("{keys} is not bound"), true);
    }

    /// Lets the panel finish background work, and says when a folder count moved.
    fn on_tick(&mut self) -> bool {
        let props = self.dirsize.as_ref().map(DirSize::state) != self.shown;
        self.panel().tick() || props
    }

    fn navigable(&mut self) -> Option<&mut dyn Navigable> {
        Some(self.tabs.active_mut())
    }

    fn tabbed(&mut self) -> Option<&mut dyn Tabbed> {
        Some(self)
    }

    fn preview_scroll(&mut self) -> Option<&mut ui::scroll::PreviewScroll> {
        Some(&mut self.scroll)
    }
}

/// The explorer's tabs are panels the user opens and closes.
impl Tabbed for Explorer {
    fn tab_next(&mut self) {
        self.tabs.select_next();
    }

    fn tab_prev(&mut self) {
        self.tabs.select_prev();
    }

    fn tab_select(&mut self, index: usize) {
        self.tabs.select_at(index);
    }

    fn can_open_close_tabs(&self) -> bool {
        true
    }

    fn tab_open(&mut self) {
        let root = self.panel().root().to_path_buf();
        self.tabs.open_tab(Panel::new(root, panel_opts(&self.opts)));
    }

    fn tab_reopen(&mut self) {
        if !self.tabs.reopen_tab() {
            self.panel().notify("no closed tab to reopen", false);
        }
    }

    fn tab_duplicate(&mut self) {
        self.tab_open();
    }

    fn tab_move(&mut self, delta: isize) {
        self.tabs.move_active_tab(delta);
    }

    fn tab_close(&mut self) {
        if !self.tabs.close_tab() {
            self.panel().notify("this is the last tab (q quits)", false);
        }
    }
}

impl Explorer {
    /// The panel of the active tab.
    fn panel(&mut self) -> &mut Panel {
        self.tabs.active_mut()
    }

    /// Carries out what the panel asked for: open, search, rename, close or a command.
    fn carry_out(&mut self, outcome: Outcome, host: &mut dyn Host) -> Request {
        match outcome {
            Outcome::Open(path) => {
                if let Some(file) = &self.opts.chooser_file {
                    if let Err(e) = fs::write(file, path.as_os_str().as_encoded_bytes()) {
                        self.panel().notify(e.to_string(), true);
                        return Request::None;
                    }
                    return Request::Quit;
                }
                let result =
                    with_suspended(host, || open(&path)).unwrap_or_else(|e| Err(e.to_string()));
                self.show_failure(result);
                self.panel().refresh();
            }
            Outcome::Close => {
                if !self.tabs.close_tab() {
                    return Request::Quit;
                }
            }
            Outcome::Search(kind) => {
                let root = self.panel().root().to_path_buf();
                if let Ok(Some(found)) = with_overlay(host, || search(&kind, &root)) {
                    self.panel().reveal(&found);
                }
            }
            Outcome::BulkRename(paths) => {
                let root = self.panel().root().to_path_buf();
                let result = with_suspended(host, || bulk_rename(&root, &paths))
                    .unwrap_or_else(|e| Err(e.to_string()));
                self.panel().clear_marks();
                self.panel().refresh();
                match result {
                    Ok(n) => self.panel().notify(
                        format!("renamed {n} item{}", if n == 1 { "" } else { "s" }),
                        false,
                    ),
                    Err(e) => self.panel().notify(e, true),
                }
            }
            Outcome::OpenWith(path, cmd) => {
                let result = with_suspended(host, || open_with(&cmd, &path))
                    .unwrap_or_else(|e| Err(e.to_string()));
                self.show_failure(result);
                self.panel().refresh();
            }
            Outcome::Command(id) => self.command(id),
            Outcome::Help => return Request::Help,
            Outcome::Palette => return Request::Palette,
            Outcome::None | Outcome::Created(..) | Outcome::Moved(_) | Outcome::Deleted(_) => {}
        }
        Request::None
    }

    /// An action of the app itself (the properties pane).
    fn command(&mut self, id: &str) {
        if id == "view.properties" {
            self.show_meta = !self.show_meta;
        }
    }

    /// Shows a failure from an external command as an error notice on the panel.
    fn show_failure(&mut self, result: Result<(), String>) {
        if let Err(e) = result {
            self.panel().notify(e, true);
        }
    }

    /// Draws the tab bar (when there are several tabs), the list and the preview.
    fn draw(&mut self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let theme = cx.theme;
        let mut body = area;
        if self.tabs.len() > 1 {
            let bar = Rect { height: 1, ..area };
            body = Rect { y: area.y + 1, height: area.height.saturating_sub(1), ..area };
            self.tab_bar(bar, buf, cx);
        }
        let (list, preview) = if self.opts.preview && body.width >= 60 {
            let [l, p] = ui::layout::main_preview(theme, body);
            (l, Some(p))
        } else {
            (body, None)
        };
        self.tabs.active_mut().render(list, buf, cx, true);
        if let Some(area) = preview {
            self.render_preview(area, buf, cx);
        }
    }

    /// Draws the tab titles, shortened so they stay unique.
    fn tab_bar(&self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let roots: Vec<&Path> = self.tabs.iter().map(|tab| tab.root()).collect();
        let labels: Vec<ui::tabs::Label> =
            ui::tabs::short_titles(&roots).into_iter().map(ui::tabs::Label::new).collect();
        ui::tabs::render(area, buf, cx.theme, &labels, self.tabs.active_index(), true);
    }

    /// Draws the preview pane for the file under the cursor, rebuilding the preview only when the
    /// file or the pane size changes.
    fn render_preview(&mut self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let current = self.tabs.active().current().map(Path::to_path_buf);
        let theme = cx.theme;
        let area = self.render_meta(area, buf, current.as_deref(), cx);
        let title = current
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let pane = match current.as_deref() {
            Some(p) if p.is_dir() => Pane::new().title(&title).icon(theme.icons.dir(&title, false)),
            Some(_) => Pane::new().title(&title).icon(theme.icons.file(&title, false)),
            None => Pane::new().title(&title),
        };
        let inner = pane.draw(cx, area, buf);
        let Some(path) = current else { return };
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let key = (path.clone(), inner.width, inner.height, pv::raw_tables());
        if self.preview.as_ref().is_none_or(|p| p.key != key) {
            // A file is drawn whole (up to `pv::SCROLLABLE_LINES`) so the pane can scroll; a folder is fitted to the pane.
            let opts = pv::Options {
                width: Some(inner.width),
                height: Some(if path.is_dir() { inner.height } else { pv::SCROLLABLE_LINES }),
                git: self.opts.git,
                focus: None,
            };
            let lines = pv::preview(&path, theme, &opts).map_err(|e| e.to_string());
            if self.preview.as_ref().is_none_or(|p| p.key.0 != key.0) {
                self.scroll.reset();
            }
            self.preview = Some(Preview { key, lines });
        }
        let lines = match &self.preview.as_ref().expect("set above").lines {
            Ok(lines) => lines.clone(),
            Err(e) => vec![Line::from(Span::styled(
                e.clone(),
                Style::new().fg(theme.colors.muted.into()),
            ))],
        };
        self.scroll.view(inner, lines.len());
        Paragraph::new(self.scroll.window(&lines).to_vec()).render(inner, buf);
    }
}

/// Guesses whether a file is text by looking for a zero byte in its first 8 KiB.
fn is_text(path: &Path) -> bool {
    let mut buf = [0u8; 8192];
    File::open(path)
        .and_then(|mut f| f.read(&mut buf))
        .map(|n| !buf[..n].contains(&0))
        .unwrap_or(false)
}

/// Open with the editor (text) or the system opener (anything else).
fn open(path: &Path) -> Result<(), String> {
    if is_text(path) {
        let editor = std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "vi".into());
        open_with(&editor, path)
    } else {
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        Command::new(opener)
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("{opener}: {e}"))
    }
}

/// Run `cmd path` through `sh` and wait for it.
fn open_with(cmd: &str, path: &Path) -> Result<(), String> {
    if cmd.trim().is_empty() {
        return Ok(());
    }
    let script = format!("{cmd} {}", ui::popup::quote(&path.to_string_lossy()));
    match Command::new("sh").arg("-c").arg(script).status() {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("{cmd} exited with {s}")),
        Err(e) => Err(format!("{cmd}: {e}")),
    }
}

/// Run `pick` over `root` and return the chosen path.
fn search(kind: &Search, root: &Path) -> Option<PathBuf> {
    let (flag, dir) = match kind {
        Search::Files => ("--files", root),
        Search::Grep => ("--grep", root),
        Search::Here(dir) => ("--files", dir.as_path()),
    };
    let mut cmd = Command::new("pick");
    cmd.arg(flag).arg(".");
    if matches!(kind, Search::Here(_)) {
        // Names in this folder only: its files and its folders, not what is below them.
        cmd.args(["--dirs", "--max-depth", "1", "--flat"]);
    }
    let out = cmd.current_dir(dir).stdin(Stdio::null()).stderr(Stdio::inherit()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next()?;
    // Grep results are `path:line:col:text`.
    let path = if *kind == Search::Grep { line.split(':').next()? } else { line };
    Some(dir.join(path.trim_end_matches('/')))
}

/// Edit the names in `$EDITOR`, then rename. Returns how many items changed.
fn bulk_rename(root: &Path, paths: &[PathBuf]) -> Result<usize, String> {
    let tmp = std::env::temp_dir().join(format!("files-rename-{}.txt", std::process::id()));
    fs::write(&tmp, ops::bulk_rename_text(root, paths)).map_err(|e| e.to_string())?;
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());
    let edited =
        open_with(&editor, &tmp).and_then(|()| fs::read_to_string(&tmp).map_err(|e| e.to_string()));
    let _ = fs::remove_file(&tmp);
    let moves = ops::plan_bulk_rename(root, paths, &edited?)?;
    ops::apply_moves(&moves).map_err(|e| e.to_string())?;
    Ok(moves.len())
}

impl Explorer {
    /// Draw the properties pane at the bottom of `area`; returns what is left for the preview.
    fn render_meta(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        current: Option<&Path>,
        cx: &ui::Context,
    ) -> Rect {
        if !self.show_meta {
            return area;
        }
        if self.meta.as_ref().map(|(p, _)| p.as_path()) != current {
            self.meta = current.map(|p| (p.to_path_buf(), Meta::read(p).ok()));
            self.dirsize = match &self.meta {
                Some((p, Some(m))) if m.is_dir => Some(DirSize::spawn(p.clone())),
                _ => None,
            };
        }
        let rows = self.meta.as_ref().and_then(|(_, m)| m.as_ref()).map_or(1, Meta::height);
        let height = (rows + 2).min(area.height / 2);
        if height < 3 {
            return area;
        }
        let split = Rect { y: area.bottom() - height, height, ..area };
        let meta = self.meta.as_ref().and_then(|(_, m)| m.as_ref());
        meta::render(split, buf, cx, meta, self.dirsize.as_ref().map(DirSize::state));
        Rect { height: area.height - height, ..area }
    }
}

/// The panel options of a tab: the standalone app's columns. The shell owns the keys, so the
/// panel's own keymap stays empty of host actions.
fn panel_opts(o: &Options) -> PanelOptions {
    PanelOptions {
        hidden: o.hidden,
        git: o.git,
        columns: files::Columns::Full,
        ..PanelOptions::default()
    }
}
