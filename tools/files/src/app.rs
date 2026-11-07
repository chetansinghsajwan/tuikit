use files::meta::{self, DirSize, Meta};
use files::{Action, Panel, PanelOptions, Search, ops};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use ui::{Pane, Theme};

/// The standalone app's own actions, added to the panel's keymap.
const ACTIONS: &[ui::keys::Action] = &[
    ui::keys::Action {
        id: "tab.new",
        name: "new tab",
        desc: "open a new tab in the same folder",
        basic: false,
        keys: &["leader t n"],
    },
    ui::keys::Action {
        id: "tab.reopen",
        name: "reopen tab",
        desc: "bring back the tab closed last",
        basic: false,
        keys: &["leader t r"],
    },
    ui::keys::Action {
        id: "tab.duplicate",
        name: "duplicate tab",
        desc: "open a copy of this tab next to it",
        basic: false,
        keys: &["leader t d"],
    },
    ui::keys::Action {
        id: "tab.move_left",
        name: "move tab left",
        desc: "move this tab one place left",
        basic: false,
        keys: &["leader t ["],
    },
    ui::keys::Action {
        id: "tab.move_right",
        name: "move tab right",
        desc: "move this tab one place right",
        basic: false,
        keys: &["leader t ]"],
    },
    ui::keys::Action {
        id: "tab.close",
        name: "close tab",
        desc: "close this tab (q closes it too, and quits on the last)",
        basic: false,
        keys: &["leader t c"],
    },
    ui::keys::Action {
        id: "tab.next",
        name: "next tab",
        desc: "switch to the next tab",
        basic: false,
        keys: &["]", "leader t l"],
    },
    ui::keys::Action {
        id: "tab.prev",
        name: "previous tab",
        desc: "switch to the previous tab",
        basic: false,
        keys: &["[", "leader t h"],
    },
    ui::keys::Action {
        id: "view.properties",
        name: "properties",
        desc: "show or hide the properties pane",
        basic: false,
        keys: &["leader v i"],
    },
];

pub struct Options {
    pub dir: PathBuf,
    pub chooser_file: Option<PathBuf>,
    pub cwd_file: Option<PathBuf>,
    pub hidden: bool,
    pub preview: bool,
    pub git: bool,
}

type Term = Terminal<CrosstermBackend<File>>;

/// The last preview, kept until the cursor, the pane size or the file changes.
struct Preview {
    key: (PathBuf, u16, u16, bool),
    lines: Result<Vec<Line<'static>>, String>,
}

struct App {
    opts: Options,
    theme: Theme,
    tabs: ui::tabs::Tabs<Panel>,
    preview: Option<Preview>,
    show_meta: bool,
    /// The keys help, while it is open.
    help: Option<ui::keys::Help>,
    meta: Option<(PathBuf, Option<Meta>)>,
    dirsize: Option<DirSize>,
}

pub fn run(opts: Options) -> io::Result<u8> {
    let dir = opts.dir.canonicalize()?;
    let panel_opts = panel_opts(&opts);
    let mut app = App {
        theme: Theme::load(),
        tabs: ui::tabs::Tabs::new(Panel::new(dir, panel_opts)),
        preview: None,
        help: None,
        show_meta: true,
        meta: None,
        dirsize: None,
        opts,
    };

    // Draw on the controlling terminal so stdout stays free for the caller.
    let tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
    let mut term: Term = Terminal::new(CrosstermBackend::new(tty))?;
    enter(&mut term)?;
    let result = app.event_loop(&mut term);
    let _ = leave(&mut term);
    if let Some(file) = &app.opts.cwd_file {
        fs::write(file, app.tabs.active().root().as_os_str().as_encoded_bytes())?;
    }
    result
}

fn enter(term: &mut Term) -> io::Result<()> {
    enable_raw_mode()?;
    execute!(term.backend_mut(), EnterAlternateScreen, Clear(ClearType::All))?;
    term.clear()
}

fn leave(term: &mut Term) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)
}

/// Run `f`, which opens `pick`: inside zellij that is a floating pane over this screen, so the
/// screen stays; elsewhere `pick` takes the terminal, so give it up first.
fn overlay<T>(term: &mut Term, f: impl FnOnce() -> T) -> io::Result<T> {
    if ui::popup::wanted(false) { Ok(f()) } else { suspended(term, f) }
}

/// Give the terminal to `f` (an editor, `pick`, ...) and take it back afterwards.
fn suspended<T>(term: &mut Term, f: impl FnOnce() -> T) -> io::Result<T> {
    leave(term)?;
    let out = f();
    enter(term)?;
    Ok(out)
}

impl App {
    fn panel(&mut self) -> &mut Panel {
        self.tabs.active_mut()
    }

    fn event_loop(&mut self, term: &mut Term) -> io::Result<u8> {
        let theme = self.theme.clone();
        let mut dirty = true;
        let mut shown = None;
        loop {
            if dirty {
                term.draw(|f| {
                    let area = f.area();
                    self.render(area, f.buffer_mut());
                    ui::anim::finish(f.buffer_mut(), &theme);
                })?;
                shown = self.dirsize.as_ref().map(DirSize::state);
            }
            if !event::poll(ui::anim::poll_timeout(Duration::from_millis(100)))? {
                // A folder size still being counted changes what is on screen.
                let props = self.dirsize.as_ref().map(DirSize::state) != shown;
                dirty = self.panel().tick() || props || ui::anim::take_redraw();
                continue;
            }
            let ev = event::read()?;
            dirty = true;
            if matches!(ev, Event::Resize(..)) {
                continue;
            }
            if let Some(code) = self.on_event(&ev, term)? {
                return Ok(code);
            }
        }
    }

    /// Handle one event; `Some(code)` quits.
    fn on_event(&mut self, ev: &Event, term: &mut Term) -> io::Result<Option<u8>> {
        let Event::Key(KeyEvent { code, modifiers, kind, .. }) = ev else { return Ok(None) };
        if *kind == KeyEventKind::Release {
            return Ok(None);
        }
        // Tab numbers are the one raw key the app keeps (not while a prompt or a leader menu is open).
        if let (Some(help), Event::Key(key)) = (self.help.as_mut(), ev) {
            if !help.key(key) {
                self.help = None;
            }
            return Ok(None);
        }
        if !self.panel().is_busy()
            && self.panel().menu().is_none()
            && !modifiers.contains(KeyModifiers::CONTROL)
            && let KeyCode::Char(c @ '1'..='9') = code
        {
            let i = *c as usize - '1' as usize;
            self.tabs.goto(i);
            return Ok(None);
        }
        let action = self.panel().handle_event(ev);
        self.act(action, term)
    }

    fn act(&mut self, action: Action, term: &mut Term) -> io::Result<Option<u8>> {
        match action {
            Action::Open(path) => {
                if let Some(file) = &self.opts.chooser_file {
                    fs::write(file, path.as_os_str().as_encoded_bytes())?;
                    return Ok(Some(0));
                }
                let result = suspended(term, || open(&path))?;
                self.report(result);
                self.panel().refresh();
            }
            Action::Close => {
                if self.tabs.close().is_none() {
                    return Ok(Some(0));
                }
            }
            Action::Search(kind) => {
                let root = self.panel().root().to_path_buf();
                if let Some(found) = overlay(term, || search(&kind, &root))? {
                    self.panel().reveal(&found);
                }
            }
            Action::BulkRename(paths) => {
                let root = self.panel().root().to_path_buf();
                let result = suspended(term, || bulk_rename(&root, &paths))?;
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
            Action::OpenWith(path, cmd) => {
                let result = suspended(term, || open_with(&cmd, &path))?;
                self.report(result);
                self.panel().refresh();
            }
            Action::Command(id) => self.command(id),
            Action::Help => self.help = Some(ui::keys::Help::default()),
            Action::Palette => {
                let rows = self.tabs.active().keys_rows();
                if let Some(id) = overlay(term, || ui::keys::choose("files", &rows, "Commands"))? {
                    let action = self.panel().run(id);
                    return self.act(action, term);
                }
            }
            Action::None | Action::Created(..) | Action::Moved(_) | Action::Deleted(_) => {}
        }
        Ok(None)
    }

    /// An action of the app itself (tabs, the properties pane).
    fn command(&mut self, id: &str) {
        match id {
            "tab.new" => {
                let root = self.panel().root().to_path_buf();
                self.tabs.open(Panel::new(root, panel_opts(&self.opts)));
            }
            "tab.reopen" => {
                if !self.tabs.reopen() {
                    self.panel().notify("no closed tab to reopen", false);
                }
            }
            "tab.duplicate" => {
                let root = self.panel().root().to_path_buf();
                self.tabs.open(Panel::new(root, panel_opts(&self.opts)));
            }
            "tab.move_left" => self.tabs.move_active(-1),
            "tab.move_right" => self.tabs.move_active(1),
            "tab.close" => {
                if self.tabs.close().is_none() {
                    self.panel().notify("this is the last tab (q quits)", false);
                }
            }
            "tab.next" => self.tabs.next(),
            "tab.prev" => self.tabs.prev(),
            "view.properties" => self.show_meta = !self.show_meta,
            _ => {}
        }
    }

    fn report(&mut self, result: Result<(), String>) {
        if let Err(e) = result {
            self.panel().notify(e, true);
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let mut body = area;
        if self.tabs.len() > 1 {
            let bar = Rect { height: 1, ..area };
            body = Rect { y: area.y + 1, height: area.height.saturating_sub(1), ..area };
            self.tab_bar(bar, buf);
        }
        let (list, preview) = if self.opts.preview && body.width >= 60 {
            let [l, p] = ui::layout::main_preview(&theme, body);
            (l, Some(p))
        } else {
            (body, None)
        };
        self.tabs.active_mut().render(list, buf, &theme, true);
        if let Some(area) = preview {
            self.render_preview(area, buf);
        }
        if let Some(menu) = self.tabs.active().menu() {
            ui::keys::render_menu(area, buf, &theme, &menu);
        }
        if let Some(help) = self.help.as_mut() {
            let rows = self.tabs.active().keys_rows();
            help.render(area, buf, &theme, "Keys", &rows);
        }
    }

    fn tab_bar(&self, area: Rect, buf: &mut Buffer) {
        let roots: Vec<&Path> = self.tabs.iter().map(|tab| tab.root()).collect();
        let labels: Vec<ui::tabs::Label> =
            ui::tabs::short_titles(&roots).into_iter().map(ui::tabs::Label::new).collect();
        ui::tabs::render(area, buf, &self.theme, &labels, self.tabs.active_index(), true);
    }

    fn render_preview(&mut self, area: Rect, buf: &mut Buffer) {
        let current = self.tabs.active().current().map(Path::to_path_buf);
        let theme = self.theme.clone();
        let area = self.render_meta(area, buf, current.as_deref());
        let title = current
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let pane = match current.as_deref() {
            Some(p) if p.is_dir() => {
                Pane::new(&theme).title(&title).icon(ui::icons::dir(&title, false))
            }
            Some(_) => Pane::new(&theme).title(&title).icon(ui::icons::file(&title, false)),
            None => Pane::new(&theme).title(&title),
        };
        let inner = pane.inner(area);
        pane.render(area, buf);
        let Some(path) = current else { return };
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let key = (path.clone(), inner.width, inner.height, pv::raw_tables());
        if self.preview.as_ref().is_none_or(|p| p.key != key) {
            let opts = pv::Options {
                width: Some(inner.width),
                height: Some(inner.height),
                git: self.opts.git,
                focus: None,
            };
            let lines = pv::preview(&path, &theme, &opts).map_err(|e| e.to_string());
            self.preview = Some(Preview { key, lines });
        }
        let lines = match &self.preview.as_ref().expect("set above").lines {
            Ok(lines) => lines.clone(),
            Err(e) => vec![Line::from(Span::styled(
                e.clone(),
                Style::new().fg(theme.colors.muted.into()),
            ))],
        };
        Paragraph::new(lines).render(inner, buf);
    }
}

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

impl App {
    /// Draw the properties pane at the bottom of `area`; returns what is left for the preview.
    fn render_meta(&mut self, area: Rect, buf: &mut Buffer, current: Option<&Path>) -> Rect {
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
        meta::render(split, buf, &self.theme, meta, self.dirsize.as_ref().map(DirSize::state));
        Rect { height: area.height - height, ..area }
    }
}

/// The panel options of a tab: the standalone app's columns and its own actions.
fn panel_opts(o: &Options) -> PanelOptions {
    PanelOptions {
        hidden: o.hidden,
        git: o.git,
        linemode: files::Linemode::Full,
        actions: ACTIONS.to_vec(),
        ..PanelOptions::default()
    }
}
