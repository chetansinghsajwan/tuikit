use super::chord::Chord;
use super::commands::{Action, Commands, Group};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

/// What a key press did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feed {
    /// A binding completed: run this action.
    Run(&'static str),
    /// A sequence is under way (see `Keymap::menu`).
    Pending,
    /// A started sequence was cancelled with Esc: the key is used up.
    Cancelled,
    /// A started sequence met a key nothing is bound to (`Keymap::rejected` names the sequence): the key is used up.
    NoBinding,
    /// Not bound: the key is the caller's.
    Unbound,
}

/// Resolves key presses to action ids, tracking a multi-key sequence that is under way.
#[derive(Debug, Clone)]
pub struct Keymap {
    pub(super) leader: Chord,
    pub(super) actions: Vec<Action>,
    pub(super) groups: Vec<(Vec<Chord>, &'static str)>,
    pub(super) bindings: Vec<(Vec<Chord>, &'static str)>,
    pub(super) pending: Vec<Chord>,
    /// The last sequence that ended on a key nothing was bound to.
    rejected: String,
}

impl Keymap {
    /// A keymap for `commands`, with `leader` (a key name) standing for `leader` in bindings; the
    /// user's configuration (`ui::config`) overrides the leader and the keys.
    pub fn new(leader: &str, commands: &Commands, groups: &[Group]) -> Keymap {
        let leader = crate::config::get()
            .leader
            .as_deref()
            .and_then(Chord::parse)
            .or_else(|| Chord::parse(leader))
            .unwrap_or(Chord { code: KeyCode::Char(' '), ctrl: false, alt: false });
        let mut map = Keymap {
            leader,
            actions: commands.actions.to_vec(),
            groups: Vec::new(),
            bindings: Vec::new(),
            pending: Vec::new(),
            rejected: String::new(),
        };
        for g in groups {
            if let Some(seq) = map.sequence(g.prefix) {
                map.groups.push((seq, g.name));
            }
        }
        for b in &commands.default_bindings {
            if !map.actions.iter().any(|a| a.id == b.id) {
                continue;
            }
            for k in b.keys {
                if let Some(seq) = map.sequence(k) {
                    map.bindings.push((seq, b.id));
                }
            }
        }
        map.apply_config();
        map
    }

    /// The user's leader and bindings (`ui::config`).
    fn apply_config(&mut self) {
        let cfg = crate::config::get();
        for (id, keys) in &cfg.keys {
            let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
            self.rebind(id, &keys);
        }
    }

    /// Read a binding such as `leader v c` into its chords, with `leader` replaced by the
    /// leader key.
    fn sequence(&self, s: &str) -> Option<Vec<Chord>> {
        s.split_whitespace()
            .map(|w| if w == "leader" { Some(self.leader) } else { Chord::parse(w) })
            .collect()
    }

    /// Replace the bindings of `id` (user configuration). Unknown ids and unparsable keys are ignored.
    pub fn rebind(&mut self, id: &str, keys: &[&str]) {
        if !self.actions.iter().any(|a| a.id == id) {
            return;
        }
        self.bindings.retain(|(_, i)| *i != id);
        for k in keys {
            // A key the user gives to this action is no longer another one's.
            if let Some(seq) = self.sequence(k) {
                self.bindings.retain(|(s, i)| !(*s == seq && *i != id));
            }
            if let (Some(seq), Some(a)) =
                (self.sequence(k), self.actions.iter().find(|a| a.id == id))
            {
                self.bindings.push((seq, a.id));
            }
        }
    }

    /// Every action the keymap was built with, for the palette and the help.
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// The keys bound to `id`, as shown (`space v c`, `j / down`).
    pub fn keys_of(&self, id: &str) -> String {
        let shown: Vec<String> = self
            .bindings
            .iter()
            .filter(|(_, i)| *i == id)
            .map(|(seq, _)| seq.iter().map(Chord::label).collect::<Vec<_>>().join(" "))
            .collect();
        shown.join(" / ")
    }

    /// The sequence behind the last `Feed::NoBinding` (`space z`), for the warning.
    pub fn rejected(&self) -> &str {
        &self.rejected
    }

    /// Whether a multi-key sequence has been started.
    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Drop a started sequence, for when the tool closes the menu another way.
    pub fn cancel(&mut self) {
        self.pending.clear();
    }

    /// Take one key press: run a finished binding, continue or cancel a sequence, or return the
    /// key to the caller.
    pub fn feed(&mut self, key: &KeyEvent) -> Feed {
        let chord = Chord::from_event(key);
        if !self.pending.is_empty() && chord.code == KeyCode::Esc && !chord.ctrl && !chord.alt {
            self.pending.clear();
            return Feed::Cancelled;
        }
        self.pending.push(chord);
        if let Some((_, id)) = self.bindings.iter().find(|(seq, _)| *seq == self.pending) {
            self.pending.clear();
            return Feed::Run(id);
        }
        let n = self.pending.len();
        let longer =
            self.bindings.iter().any(|(seq, _)| seq.len() > n && seq[..n] == self.pending[..]);
        if longer {
            return Feed::Pending;
        }
        if n > 1 {
            // A key nothing is bound to inside a menu: name it, drop it, and keep the menu open.
            self.rejected = self.pending.iter().map(Chord::label).collect::<Vec<_>>().join(" ");
            self.pending.pop();
            return Feed::NoBinding;
        }
        self.pending.clear();
        Feed::Unbound
    }
}
