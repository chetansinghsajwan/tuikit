//! The app shell: the terminal, the keymap, the event loop and the overlays every full-screen
//! tool shares (leader menu, keys help, command palette). A tool implements `Screen`, lists its
//! own actions on an `Shell` and runs it, so none of them writes its own setup, loop, help or
//! suspend logic. The shell registers the shared actions itself (movement, palette, help), so the
//! help and the palette look the same everywhere.

use crate::Theme;
use crate::keys::{
    Action, Feed, Group, Help, Keymap, Menu, Row, action, basic, choose, render_menu,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::layout::Rect;
use std::fs::{File, OpenOptions};
use std::io;
use std::time::Duration;

/// The terminal the tools draw on, backed by `/dev/tty` so stdout stays free for the caller.
pub type Term = Terminal<CrosstermBackend<File>>;

/// What a screen asks the shell to do after it handled an event or ran an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    None,
    /// Leave the loop.
    Quit,
    /// Open the keys help.
    Help,
    /// Open the command palette.
    Palette,
}

/// The terminal as a screen may use it while it handles an event: to hand the terminal to an
/// editor or to open the picker over the screen. Kept apart from `Term` so tests need no terminal.
pub trait Host {
    /// Gives the terminal up while `f` runs (an editor, a pager) and takes it back.
    fn suspend(&mut self, f: &mut dyn FnMut()) -> io::Result<()>;

    /// Runs `f`, which opens the picker: inside zellij that is a floating pane over the screen, so
    /// the screen stays; elsewhere the picker takes the terminal, so it is given up first.
    fn overlay(&mut self, f: &mut dyn FnMut()) -> io::Result<()>;
}

/// The whole of a tool's `main`: loads the shared config of `tool` (printing what is wrong with
/// it), runs `run`, and turns its result into the exit code. `Ok(code)` exits with `code`; an
/// error is printed as `tool: message` and exits with 2.
pub fn main<E: std::fmt::Display>(
    tool: &str,
    run: impl FnOnce() -> Result<u8, E>,
) -> std::process::ExitCode {
    for problem in crate::config::init(tool) {
        eprintln!("{tool}: {problem}");
    }
    match run() {
        Ok(code) => std::process::ExitCode::from(code),
        Err(e) => {
            eprintln!("{tool}: {e}");
            std::process::ExitCode::from(2)
        }
    }
}

/// Gives the terminal to `f` and takes it back afterwards.
pub fn with_suspended<T>(host: &mut dyn Host, f: impl FnOnce() -> T) -> io::Result<T> {
    let mut f = Some(f);
    let mut out = None;
    host.suspend(&mut || out = f.take().map(|f| f()))?;
    Ok(out.expect("the host runs the closure"))
}

/// Runs `f`, which opens the picker, over the screen where the terminal allows it.
pub fn with_overlay<T>(host: &mut dyn Host, f: impl FnOnce() -> T) -> io::Result<T> {
    let mut f = Some(f);
    let mut out = None;
    host.overlay(&mut || out = f.take().map(|f| f()))?;
    Ok(out.expect("the host runs the closure"))
}

impl Host for Term {
    fn suspend(&mut self, f: &mut dyn FnMut()) -> io::Result<()> {
        leave(self)?;
        f();
        enter(self)
    }

    fn overlay(&mut self, f: &mut dyn FnMut()) -> io::Result<()> {
        if crate::popup::wanted(false) {
            f();
            Ok(())
        } else {
            self.suspend(f)
        }
    }
}

/// A host with no terminal behind it: the closures just run. For tests.
pub struct NoHost;

impl Host for NoHost {
    fn suspend(&mut self, f: &mut dyn FnMut()) -> io::Result<()> {
        f();
        Ok(())
    }

    fn overlay(&mut self, f: &mut dyn FnMut()) -> io::Result<()> {
        f();
        Ok(())
    }
}

/// Movement along the rows of a screen. A screen that offers it gets the shared `nav.*` actions
/// (and their keys) without declaring them.
pub trait Navigable {
    /// Moves the cursor by `delta` rows, stopping at the first and the last.
    fn move_by(&mut self, delta: isize);

    /// How many rows a page move covers.
    fn page_row_count(&self) -> usize;

    /// Moves to the first row.
    fn move_to_top(&mut self);

    /// Moves to the last row.
    fn move_to_bottom(&mut self);

    /// How many rows `nav.up_step` and `nav.down_step` move.
    fn step_size(&self) -> usize {
        5
    }
}

/// Tabs of a screen. A screen that offers them gets the shared `tab.*` actions (and their keys)
/// without declaring them, and the number keys `1` to `9` go to a tab when nothing else is bound
/// to them.
pub trait Tabbed {
    /// Switches to the next tab, wrapping around.
    fn tab_next(&mut self);

    /// Switches to the previous tab, wrapping around.
    fn tab_prev(&mut self);

    /// Switches to the tab at `index` (0 for the first); out of range does nothing.
    fn tab_select(&mut self, index: usize);

    /// Whether the user opens and closes tabs (a file explorer) or the tabs are fixed (the Status,
    /// Commits and Refs tabs of a git screen). Only dynamic tabs get the actions below.
    fn can_open_close_tabs(&self) -> bool {
        false
    }

    /// Opens a new tab where this one is.
    fn tab_open(&mut self) {}

    /// Brings back the tab closed last.
    fn tab_reopen(&mut self) {}

    /// Opens a copy of this tab next to it.
    fn tab_duplicate(&mut self) {}

    /// Moves this tab by `delta` places.
    fn tab_move(&mut self, _delta: isize) {}

    /// Closes this tab.
    fn tab_close(&mut self) {}
}

/// What a tool provides to the shell: how it draws and how it runs its own actions. The shell
/// owns the keymap, so it decides which key means which action.
pub trait Screen {
    /// Draws the whole screen.
    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme);

    /// Whether this event is the screen's own and the keymap, leader included, stays out of it:
    /// typed text in a prompt, an answer to a question.
    fn captures_input(&self, _ev: &Event) -> bool {
        false
    }

    /// Reacts to an event the keymap did not turn into an action: events `captures` claimed,
    /// keys nothing is bound to, and every other event.
    fn on_event(&mut self, _ev: &Event, _host: &mut dyn Host) -> Request {
        Request::None
    }

    /// Runs one of the tool's own actions by id (also what a palette pick resolves to).
    fn run_action(&mut self, id: &'static str, host: &mut dyn Host) -> Request;

    /// Called for each key press the keymap is about to decide on (not for keys the screen
    /// captures): a place to clear a message left by the last action.
    fn on_keypress(&mut self) {}

    /// A key sequence ended on a key nothing is bound to; `keys` names the sequence.
    fn on_unbound_keys(&mut self, _keys: &str) {}

    /// Called when no input came for a tick: background work and clocks go here. Returns whether
    /// the screen changed and must be drawn again.
    fn on_tick(&mut self) -> bool {
        false
    }

    /// The movement the screen offers, if any.
    fn navigable(&mut self) -> Option<&mut dyn Navigable> {
        None
    }

    /// The tabs the screen offers, if any.
    fn tabbed(&mut self) -> Option<&mut dyn Tabbed> {
        None
    }

    /// Which set of `Shell::contexts` is active (the focused pane): its actions and keys apply now.
    fn context(&self) -> usize {
        0
    }
}

/// The shared movement actions, registered when the screen is `Navigable`. The keys follow
/// `docs/usage-design.md`: `ctrl-d` and `ctrl-u` are left to the preview scroll.
const NAV: &[Action] = &[
    basic("nav.up", "up", "move the cursor up", &["k", "up", "ctrl-p"]),
    basic("nav.down", "down", "move the cursor down", &["j", "down", "ctrl-n"]),
    basic("nav.up_step", "up a step", "move the cursor up several rows", &["ctrl-k"]),
    basic("nav.down_step", "down a step", "move the cursor down several rows", &["ctrl-j"]),
    basic("nav.page_up", "page up", "move a page up", &["pageup"]),
    basic("nav.page_down", "page down", "move a page down", &["pagedown"]),
    basic("nav.top", "top", "go to the first row", &["g", "home"]),
    basic("nav.bottom", "bottom", "go to the last row", &["G", "end"]),
];

/// The tab actions of every screen with tabs.
const TABS: &[Action] = &[
    basic("tab.next", "next tab", "switch to the next tab", &["]", "leader t l"]),
    basic("tab.prev", "previous tab", "switch to the previous tab", &["[", "leader t h"]),
];

/// The extra tab actions of a screen whose tabs the user opens and closes.
const DYNAMIC_TABS: &[Action] = &[
    action("tab.new", "new tab", "open a new tab in the same place", &["leader t n"]),
    action("tab.reopen", "reopen tab", "bring back the tab closed last", &["leader t r"]),
    action("tab.duplicate", "duplicate tab", "open a copy of this tab next to it", &["leader t d"]),
    action("tab.move_left", "move tab left", "move this tab one place left", &["leader t ["]),
    action("tab.move_right", "move tab right", "move this tab one place right", &["leader t ]"]),
    action("tab.close", "close tab", "close this tab", &["leader t c"]),
];

/// The name of the leader menu behind `leader t`.
const TAB_GROUP: Group = Group { prefix: "leader t", name: "tabs" };

/// The actions of the shell itself, on every screen.
const SHELL: &[Action] = &[
    basic("palette", "commands", "search every action by name and run it", &["leader space"]),
    basic("help", "keys", "list every action with its keys", &["?"]),
];

/// How a tool wants its app set up: its name, its own actions and which shared parts it uses.
pub struct Shell {
    tool: &'static str,
    leader: &'static str,
    actions: Vec<Action>,
    contexts: Vec<Vec<Action>>,
    groups: &'static [Group],
    nav: bool,
    tabs: bool,
    tick: Duration,
}

impl Shell {
    /// An app for `tool` (it names the palette) with the default leader, no actions of its own,
    /// the shared movement on and a tick of 250 ms.
    pub fn new(tool: &'static str) -> Shell {
        Shell {
            tool,
            leader: "space",
            actions: Vec::new(),
            contexts: Vec::new(),
            groups: &[],
            nav: true,
            tabs: true,
            tick: Duration::from_millis(250),
        }
    }

    /// The key that starts the leader menu, unless the user's configuration sets another.
    pub fn leader(mut self, leader: &'static str) -> Shell {
        self.leader = leader;
        self
    }

    /// The tool's own actions.
    pub fn actions(mut self, actions: impl Into<Vec<Action>>) -> Shell {
        self.actions = actions.into();
        self
    }

    /// Extra actions that apply only while a context is active (a focused pane), one set each;
    /// `Screen::context` picks the set. Each context gets its own keymap, so the same key may mean
    /// different things in different contexts.
    pub fn contexts(mut self, contexts: impl Into<Vec<Vec<Action>>>) -> Shell {
        self.contexts = contexts.into();
        self
    }

    /// Names for the tool's leader-menu prefixes.
    pub fn groups(mut self, groups: &'static [Group]) -> Shell {
        self.groups = groups;
        self
    }

    /// Leaves the shared movement out, even for a screen that offers it.
    pub fn with_navigation(mut self, on: bool) -> Shell {
        self.nav = on;
        self
    }

    /// Leaves the shared tab actions out, even for a screen that offers tabs.
    pub fn with_tabs(mut self, on: bool) -> Shell {
        self.tabs = on;
        self
    }

    /// How long the loop waits for input before it calls `Screen::tick`.
    pub fn tick_interval(mut self, tick: Duration) -> Shell {
        self.tick = tick;
        self
    }

    /// Builds the keymaps (one per context) from the shared actions, the tool's and the user's
    /// configuration. Panics when two actions of a keymap share an id, so a clash shows up at
    /// startup, not as a dead key.
    pub fn dispatcher<S: Screen>(&self, screen: &mut S) -> Dispatcher {
        let nav = self.nav && screen.navigable().is_some();
        let tabs = self.tabs && screen.tabbed().is_some();
        let mut all: Vec<Action> = Vec::new();
        if nav {
            all.extend_from_slice(NAV);
        }
        if tabs {
            all.extend_from_slice(TABS);
            if screen.tabbed().is_some_and(|t| t.can_open_close_tabs()) {
                all.extend_from_slice(DYNAMIC_TABS);
            }
        }
        let mut groups = self.groups.to_vec();
        if tabs && groups.iter().all(|g| g.prefix != TAB_GROUP.prefix) {
            groups.push(TAB_GROUP);
        }
        all.extend_from_slice(SHELL);
        all.extend(self.actions.iter().cloned());
        let none = [Vec::new()];
        let contexts = if self.contexts.is_empty() { &none[..] } else { &self.contexts };
        let keymaps = contexts
            .iter()
            .map(|own| {
                let mut all = all.clone();
                all.extend(own.iter().cloned());
                for (i, a) in all.iter().enumerate() {
                    assert!(
                        all[..i].iter().all(|b| b.id != a.id),
                        "action {} is registered twice",
                        a.id
                    );
                }
                Keymap::new(self.leader, &all, &groups)
            })
            .collect();
        Dispatcher { keymaps, nav, tabs }
    }

    /// Runs the screen until it quits and always restores the terminal afterwards.
    pub fn run<S: Screen>(self, screen: &mut S) -> io::Result<()> {
        let mut dispatcher = self.dispatcher(screen);
        let mut term = open()?;
        enter(&mut term)?;
        let result = event_loop(&self, &mut dispatcher, &mut term, screen);
        let _ = leave(&mut term);
        result
    }
}

/// Turns events into actions: the keymap decides, the shell runs the shared actions and the
/// screen the rest.
pub struct Dispatcher {
    /// One per context, the first alone when the tool has none.
    keymaps: Vec<Keymap>,
    nav: bool,
    tabs: bool,
}

impl Dispatcher {
    /// Handles one event: a screen that claims it gets it as is, else keys go through the keymap.
    pub fn handle_event<S: Screen>(
        &mut self,
        screen: &mut S,
        ev: &Event,
        host: &mut dyn Host,
    ) -> Request {
        let Event::Key(key) = ev else { return screen.on_event(ev, host) };
        if key.kind == KeyEventKind::Release {
            return Request::None;
        }
        if screen.captures_input(ev) {
            return screen.on_event(ev, host);
        }
        screen.on_keypress();
        let ctx = self.context_index(screen.context());
        match self.keymaps[ctx].feed(key) {
            Feed::Run(id) => self.run_action(screen, id, host),
            Feed::NoBinding => {
                screen.on_unbound_keys(self.keymaps[ctx].rejected());
                Request::None
            }
            Feed::Unbound => {
                if self.tabs
                    && let KeyCode::Char(c @ '1'..='9') = key.code
                    && !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && let Some(tabs) = screen.tabbed()
                {
                    tabs.tab_select(c as usize - '1' as usize);
                    return Request::None;
                }
                screen.on_event(ev, host)
            }
            Feed::Pending | Feed::Cancelled => Request::None,
        }
    }

    /// Runs an action by id: the shared ones here, the tool's own on the screen.
    pub fn run_action<S: Screen>(
        &mut self,
        screen: &mut S,
        id: &'static str,
        host: &mut dyn Host,
    ) -> Request {
        match id {
            "palette" => Request::Palette,
            "help" => Request::Help,
            _ if self.nav && NAV.iter().any(|a| a.id == id) => {
                if let Some(nav) = screen.navigable() {
                    run_nav_action(nav, id);
                }
                Request::None
            }
            _ if self.tabs && id.starts_with("tab.") => {
                if let Some(tabs) = screen.tabbed() {
                    run_tab_action(tabs, id);
                }
                Request::None
            }
            _ => screen.run_action(id, host),
        }
    }

    /// The keymap of a context, clamped so a screen reporting a context it did not register still
    /// gets one.
    fn context_index(&self, ctx: usize) -> usize {
        ctx.min(self.keymaps.len() - 1)
    }

    /// The pending key-prefix menu of a context, to draw over the screen.
    pub fn leader_menu(&self, ctx: usize) -> Option<Menu> {
        self.keymaps[self.context_index(ctx)].menu()
    }

    /// Every action of a context with its keys, for the help and the palette.
    pub fn palette_rows(&self, ctx: usize) -> Vec<Row> {
        self.keymaps[self.context_index(ctx)].rows()
    }
}

/// Carries out one shared tab action on the screen.
fn run_tab_action(tabs: &mut dyn Tabbed, id: &str) {
    match id {
        "tab.next" => tabs.tab_next(),
        "tab.prev" => tabs.tab_prev(),
        "tab.new" => tabs.tab_open(),
        "tab.reopen" => tabs.tab_reopen(),
        "tab.duplicate" => tabs.tab_duplicate(),
        "tab.move_left" => tabs.tab_move(-1),
        "tab.move_right" => tabs.tab_move(1),
        "tab.close" => tabs.tab_close(),
        _ => {}
    }
}

/// Carries out one shared movement on the screen.
fn run_nav_action(nav: &mut dyn Navigable, id: &str) {
    let page = nav.page_row_count().max(1) as isize;
    let step = nav.step_size().max(1) as isize;
    match id {
        "nav.up" => nav.move_by(-1),
        "nav.down" => nav.move_by(1),
        "nav.up_step" => nav.move_by(-step),
        "nav.down_step" => nav.move_by(step),
        "nav.page_up" => nav.move_by(-page),
        "nav.page_down" => nav.move_by(page),
        "nav.top" => nav.move_to_top(),
        "nav.bottom" => nav.move_to_bottom(),
        _ => {}
    }
}

/// Opens the controlling terminal, so stdout stays free for the caller.
fn open() -> io::Result<Term> {
    let tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
    Terminal::new(CrosstermBackend::new(tty))
}

/// Puts the terminal into the full-screen raw mode the tools draw in.
fn enter(term: &mut Term) -> io::Result<()> {
    enable_raw_mode()?;
    execute!(term.backend_mut(), EnterAlternateScreen, Clear(ClearType::All))?;
    // Not `Terminal::clear`: it asks the terminal for the cursor position, and that question goes
    // to stdout, which a tool that prints its result there (`pick`) has redirected.
    let area = term.size()?;
    term.resize(Rect::from(area))
}

/// Gives the terminal back to the shell.
fn leave(term: &mut Term) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)
}

/// Draws the screen and the overlays when something changed, then handles input until the screen
/// asks to quit.
fn event_loop<S: Screen>(
    app: &Shell,
    dispatcher: &mut Dispatcher,
    term: &mut Term,
    screen: &mut S,
) -> io::Result<()> {
    let theme = Theme::load();
    let mut help: Option<Help> = None;
    let mut dirty = true;
    loop {
        if dirty {
            let ctx = screen.context();
            term.draw(|f| {
                let area = f.area();
                screen.render(area, f.buffer_mut(), &theme);
                if let Some(menu) = dispatcher.leader_menu(ctx) {
                    render_menu(area, f.buffer_mut(), &theme, &menu);
                }
                if let Some(h) = help.as_mut() {
                    h.render(area, f.buffer_mut(), &theme, "Keys", &dispatcher.palette_rows(ctx));
                }
                crate::anim::finish(f.buffer_mut(), &theme);
            })?;
        }
        if !event::poll(crate::anim::poll_timeout(app.tick))? {
            dirty = screen.on_tick() || crate::anim::take_redraw();
            continue;
        }
        let ev = event::read()?;
        dirty = true;
        if matches!(ev, Event::Resize(..)) {
            continue;
        }
        // The help is modal: it takes the keys until it closes.
        if let (Some(h), Event::Key(key)) = (help.as_mut(), &ev) {
            if !h.key(key) {
                help = None;
            }
            continue;
        }
        match dispatcher.handle_event(screen, &ev, term) {
            Request::Quit => return Ok(()),
            Request::Help => help = Some(Help::default()),
            Request::Palette => {
                let rows = dispatcher.palette_rows(screen.context());
                if let Some(id) = with_overlay(term, || choose(app.tool, &rows, "Commands"))?
                    && dispatcher.run_action(screen, id, term) == Request::Quit
                {
                    return Ok(());
                }
            }
            Request::None => {}
        }
    }
}

/// Tests for the app shell.
#[cfg(test)]
mod tests;
