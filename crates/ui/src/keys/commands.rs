use super::chord::Chord;

/// A named thing a tool can do. Which keys run it is a separate matter (see [`DefaultBinding`]): an
/// action with no binding stays reachable from the command palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    /// Stable id, `area.verb`: what the tool matches on and what config refers to.
    pub id: &'static str,
    /// Short name, shown in menus and the palette.
    pub name: &'static str,
    pub desc: &'static str,
    /// Navigation and other keys everyone knows: left out of the command palette (the `?`
    /// help still lists them).
    pub basic: bool,
}

/// The default binding of an action: the keys that run it, by its id (see the module docs for the syntax). A binding
/// whose id names no registered action is ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultBinding {
    pub id: &'static str,
    pub keys: &'static [&'static str],
}

/// The actions a keymap offers and the default bindings of some of them. Tools and shells build
/// one from their tables, so actions and bindings travel together without being one thing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Commands {
    pub actions: Vec<Action>,
    pub default_bindings: Vec<DefaultBinding>,
}

impl Commands {
    /// A set from one table of actions and one of bindings.
    pub fn of(actions: &[Action], bindings: &[DefaultBinding]) -> Commands {
        let mut set = Commands::default();
        set.add(actions, bindings);
        set
    }

    /// Adds a table of actions and a table of bindings to the set.
    pub fn add(&mut self, actions: &[Action], bindings: &[DefaultBinding]) {
        self.actions.extend_from_slice(actions);
        self.default_bindings.extend_from_slice(bindings);
    }

    /// What is wrong with the set, one line each (empty when it is sound): an action with no name or
    /// description or an id used twice, a binding for no action or with no keys, a key that does not
    /// parse, or a key bound twice. Every tool's tests assert it is empty.
    pub fn problems(&self) -> Vec<String> {
        let mut found = Vec::new();
        let mut ids = std::collections::HashSet::new();
        for a in &self.actions {
            if a.name.is_empty() || a.desc.is_empty() {
                found.push(format!("{} has no name or description", a.id));
            }
            if !ids.insert(a.id) {
                found.push(format!("duplicate action {}", a.id));
            }
        }
        let mut shown = std::collections::HashSet::new();
        for b in &self.default_bindings {
            if !ids.contains(b.id) {
                found.push(format!("binding for unknown action {}", b.id));
            }
            if b.keys.is_empty() {
                found.push(format!("{} has no keys", b.id));
            }
            for k in b.keys {
                let parsable =
                    k.split_whitespace().all(|w| w == "leader" || Chord::parse(w).is_some());
                if !parsable {
                    found.push(format!("{} has an unparsable key {k}", b.id));
                }
                if !shown.insert(*k) {
                    found.push(format!("key {k} bound twice"));
                }
            }
        }
        found
    }

    /// Adds everything of `other` to the set.
    pub fn extend(&mut self, other: &Commands) {
        self.add(&other.actions, &other.default_bindings);
    }
}

/// A name for the menu behind a key prefix: `leader v` is "view".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Group {
    pub prefix: &'static str,
    pub name: &'static str,
}
