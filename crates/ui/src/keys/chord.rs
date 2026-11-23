use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// One key press, without the parts that do not matter (shift on a character is
/// already in the character).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub(super) code: KeyCode,
    pub(super) ctrl: bool,
    pub(super) alt: bool,
}

impl Chord {
    /// Read one key name such as `j`, `ctrl-d` or `enter`; `None` if it is not a known key.
    pub fn parse(s: &str) -> Option<Chord> {
        let (mut ctrl, mut alt) = (false, false);
        let mut rest = s;
        loop {
            if let Some(r) = rest.strip_prefix("ctrl-") {
                ctrl = true;
                rest = r;
            } else if let Some(r) = rest.strip_prefix("alt-") {
                alt = true;
                rest = r;
            } else {
                break;
            }
        }
        let code = match rest {
            "space" => KeyCode::Char(' '),
            "enter" => KeyCode::Enter,
            "esc" => KeyCode::Esc,
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "backspace" => KeyCode::Backspace,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            _ => {
                let mut chars = rest.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => KeyCode::Char(c),
                    _ => return None,
                }
            }
        };
        Some(Chord { code, ctrl, alt })
    }

    /// The chord of a real key event, so pressed keys compare with the configured ones.
    pub fn from_event(key: &KeyEvent) -> Chord {
        Chord {
            code: key.code,
            ctrl: key.modifiers.contains(KeyModifiers::CONTROL),
            alt: key.modifiers.contains(KeyModifiers::ALT),
        }
    }

    /// How the key is shown in menus and help: `j`, `space`, `ctrl-d`, `enter`.
    pub fn label(&self) -> String {
        let base = match self.code {
            KeyCode::Char(' ') => "space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Enter => "enter".into(),
            KeyCode::Esc => "esc".into(),
            KeyCode::Tab => "tab".into(),
            KeyCode::BackTab => "backtab".into(),
            KeyCode::Backspace => "backspace".into(),
            KeyCode::Up => "up".into(),
            KeyCode::Down => "down".into(),
            KeyCode::Left => "left".into(),
            KeyCode::Right => "right".into(),
            KeyCode::Home => "home".into(),
            KeyCode::End => "end".into(),
            KeyCode::PageUp => "pageup".into(),
            KeyCode::PageDown => "pagedown".into(),
            other => format!("{other:?}").to_lowercase(),
        };
        format!(
            "{}{}{base}",
            if self.ctrl { "ctrl-" } else { "" },
            if self.alt { "alt-" } else { "" }
        )
    }
}

/// The key of `ev` when it is a key press (or repeat), `None` for releases and other events. Terminals
/// that report releases send one per key, which no tool acts on.
pub fn pressed(ev: &Event) -> Option<&KeyEvent> {
    match ev {
        Event::Key(key) if key.kind != KeyEventKind::Release => Some(key),
        _ => None,
    }
}
