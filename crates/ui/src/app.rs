//! The app shell: the terminal, the keymap, the event loop and the overlays every full-screen
//! tool shares (leader menu, keys help, command palette). A tool implements `Screen`, lists its
//! own actions on an `App` and runs it, so none of them writes its own setup, loop, help or
//! suspend logic. The shell registers the shared actions itself (movement, palette, help), so the
//! help and the palette look the same everywhere.

use crate::Theme;
use crate::keys::{Action, Feed, Group, Help, Keymap, Menu, Row, basic, choose, render_menu};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
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

/// Gives the terminal to `f` and takes it back afterwards.
pub fn suspended<T>(host: &mut dyn Host, f: impl FnOnce() -> T) -> io::Result<T> {
    let mut f = Some(f);
    let mut out = None;
    host.suspend(&mut || out = f.take().map(|f| f()))?;
    Ok(out.expect("the host runs the closure"))
}

/// Runs `f`, which opens the picker, over the screen where the terminal allows it.
pub fn overlay<T>(host: &mut dyn Host, f: impl FnOnce() -> T) -> io::Result<T> {
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
    fn page(&self) -> usize;

    /// Moves to the first row.
    fn top(&mut self);

    /// Moves to the last row.
    fn bottom(&mut self);

    /// How many rows the step moves (`nav.up5` and `nav.down5`) cover.
    fn step(&self) -> usize {
        5
    }
}

/// What a tool provides to the shell: how it draws and how it runs its own actions. The shell
/// owns the keymap, so it decides which key means which action.
pub trait Screen {
    /// Draws the whole screen.
    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme);

    /// Whether this event is the screen's own and the keymap, leader included, stays out of it:
    /// typed text in a prompt, an answer to a question.
    fn captures(&self, _ev: &Event) -> bool {
        false
    }

    /// Reacts to an event the keymap did not turn into an action: events `captures` claimed,
    /// keys nothing is bound to, and every other event.
    fn event(&mut self, _ev: &Event, _host: &mut dyn Host) -> Request {
        Request::None
    }

    /// Runs one of the tool's own actions by id (also what a palette pick resolves to).
    fn run(&mut self, id: &'static str, host: &mut dyn Host) -> Request;

    /// Called for each key press the keymap is about to decide on (not for keys the screen
    /// captures): a place to clear a message left by the last action.
    fn keypress(&mut self) {}

    /// A key sequence ended on a key nothing is bound to; `keys` names the sequence.
    fn unbound(&mut self, _keys: &str) {}

    /// Called when no input came for a tick: background work and clocks go here. Returns whether
    /// the screen changed and must be drawn again.
    fn tick(&mut self) -> bool {
        false
    }

    /// The movement the screen offers, if any.
    fn nav(&mut self) -> Option<&mut dyn Navigable> {
        None
    }

    /// Which set of `App::contexts` is active (the focused pane): its actions and keys apply now.
    fn context(&self) -> usize {
        0
    }
}

/// The shared movement actions, registered when the screen is `Navigable`. The keys follow
/// `docs/usage-design.md`: `ctrl-d` and `ctrl-u` are left to the preview scroll.
const NAV: &[Action] = &[
    basic("nav.up", "up", "move the cursor up", &["k", "up", "ctrl-p"]),
    basic("nav.down", "down", "move the cursor down", &["j", "down", "ctrl-n"]),
    basic("nav.up5", "up a step", "move the cursor up several rows", &["ctrl-k"]),
    basic("nav.down5", "down a step", "move the cursor down several rows", &["ctrl-j"]),
    basic("nav.page_up", "page up", "move a page up", &["pageup"]),
    basic("nav.page_down", "page down", "move a page down", &["pagedown"]),
    basic("nav.top", "top", "go to the first row", &["g", "home"]),
    basic("nav.bottom", "bottom", "go to the last row", &["G", "end"]),
];

/// The actions of the shell itself, on every screen.
const SHELL: &[Action] = &[
    basic("palette", "commands", "search every action by name and run it", &["leader space"]),
    basic("help", "keys", "list every action with its keys", &["?"]),
];

/// How a tool wants its app set up: its name, its own actions and which shared parts it uses.
pub struct App {
    tool: &'static str,
    leader: &'static str,
    actions: Vec<Action>,
    contexts: Vec<Vec<Action>>,
    groups: &'static [Group],
    nav: bool,
    tick: Duration,
}

impl App {
    /// An app for `tool` (it names the palette) with the default leader, no actions of its own,
    /// the shared movement on and a tick of 250 ms.
    pub fn new(tool: &'static str) -> App {
        App {
            tool,
            leader: "space",
            actions: Vec::new(),
            contexts: Vec::new(),
            groups: &[],
            nav: true,
            tick: Duration::from_millis(250),
        }
    }

    /// The key that starts the leader menu, unless the user's configuration sets another.
    pub fn leader(mut self, leader: &'static str) -> App {
        self.leader = leader;
        self
    }

    /// The tool's own actions.
    pub fn actions(mut self, actions: impl Into<Vec<Action>>) -> App {
        self.actions = actions.into();
        self
    }

    /// Extra actions that apply only while a context is active (a focused pane), one set each;
    /// `Screen::context` picks the set. Each context gets its own keymap, so the same key may mean
    /// different things in different contexts.
    pub fn contexts(mut self, contexts: impl Into<Vec<Vec<Action>>>) -> App {
        self.contexts = contexts.into();
        self
    }

    /// Names for the tool's leader-menu prefixes.
    pub fn groups(mut self, groups: &'static [Group]) -> App {
        self.groups = groups;
        self
    }

    /// Leaves the shared movement out, even for a screen that offers it.
    pub fn no_nav(mut self) -> App {
        self.nav = false;
        self
    }

    /// How long the loop waits for input before it calls `Screen::tick`.
    pub fn tick(mut self, tick: Duration) -> App {
        self.tick = tick;
        self
    }

    /// Builds the keymaps (one per context) from the shared actions, the tool's and the user's
    /// configuration. Panics when two actions of a keymap share an id, so a clash shows up at
    /// startup, not as a dead key.
    pub fn dispatcher<S: Screen>(&self, screen: &mut S) -> Dispatcher {
        let nav = self.nav && screen.nav().is_some();
        let mut all: Vec<Action> = Vec::new();
        if nav {
            all.extend_from_slice(NAV);
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
                Keymap::new(self.leader, &all, self.groups)
            })
            .collect();
        Dispatcher { keymaps, nav }
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
}

impl Dispatcher {
    /// Handles one event: a screen that claims it gets it as is, else keys go through the keymap.
    pub fn event<S: Screen>(&mut self, screen: &mut S, ev: &Event, host: &mut dyn Host) -> Request {
        let Event::Key(key) = ev else { return screen.event(ev, host) };
        if key.kind == KeyEventKind::Release {
            return Request::None;
        }
        if screen.captures(ev) {
            return screen.event(ev, host);
        }
        screen.keypress();
        let ctx = self.index(screen.context());
        match self.keymaps[ctx].feed(key) {
            Feed::Run(id) => self.run(screen, id, host),
            Feed::NoBinding => {
                screen.unbound(self.keymaps[ctx].rejected());
                Request::None
            }
            Feed::Unbound => screen.event(ev, host),
            Feed::Pending | Feed::Cancelled => Request::None,
        }
    }

    /// Runs an action by id: the shared ones here, the tool's own on the screen.
    pub fn run<S: Screen>(
        &mut self,
        screen: &mut S,
        id: &'static str,
        host: &mut dyn Host,
    ) -> Request {
        match id {
            "palette" => Request::Palette,
            "help" => Request::Help,
            _ if self.nav && NAV.iter().any(|a| a.id == id) => {
                if let Some(nav) = screen.nav() {
                    navigate(nav, id);
                }
                Request::None
            }
            _ => screen.run(id, host),
        }
    }

    /// The keymap of a context, clamped so a screen reporting a context it did not register still
    /// gets one.
    fn index(&self, ctx: usize) -> usize {
        ctx.min(self.keymaps.len() - 1)
    }

    /// The pending key-prefix menu of a context, to draw over the screen.
    pub fn menu(&self, ctx: usize) -> Option<Menu> {
        self.keymaps[self.index(ctx)].menu()
    }

    /// Every action of a context with its keys, for the help and the palette.
    pub fn rows(&self, ctx: usize) -> Vec<Row> {
        self.keymaps[self.index(ctx)].rows()
    }
}

/// Carries out one shared movement on the screen.
fn navigate(nav: &mut dyn Navigable, id: &str) {
    let page = nav.page().max(1) as isize;
    let step = nav.step().max(1) as isize;
    match id {
        "nav.up" => nav.move_by(-1),
        "nav.down" => nav.move_by(1),
        "nav.up5" => nav.move_by(-step),
        "nav.down5" => nav.move_by(step),
        "nav.page_up" => nav.move_by(-page),
        "nav.page_down" => nav.move_by(page),
        "nav.top" => nav.top(),
        "nav.bottom" => nav.bottom(),
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
    term.clear()
}

/// Gives the terminal back to the shell.
fn leave(term: &mut Term) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)
}

/// Draws the screen and the overlays when something changed, then handles input until the screen
/// asks to quit.
fn event_loop<S: Screen>(
    app: &App,
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
                if let Some(menu) = dispatcher.menu(ctx) {
                    render_menu(area, f.buffer_mut(), &theme, &menu);
                }
                if let Some(h) = help.as_mut() {
                    h.render(area, f.buffer_mut(), &theme, "Keys", &dispatcher.rows(ctx));
                }
                crate::anim::finish(f.buffer_mut(), &theme);
            })?;
        }
        if !event::poll(crate::anim::poll_timeout(app.tick))? {
            dirty = screen.tick() || crate::anim::take_redraw();
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
        match dispatcher.event(screen, &ev, term) {
            Request::Quit => return Ok(()),
            Request::Help => help = Some(Help::default()),
            Request::Palette => {
                let rows = dispatcher.rows(screen.context());
                if let Some(id) = overlay(term, || choose(app.tool, &rows, "Commands"))?
                    && dispatcher.run(screen, id, term) == Request::Quit
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
