//! The part of a kanata `.kbd` this tool understands: the physical layout (`defsrc`), the
//! aliases and the layers written with `deflayermap`.

use std::collections::HashMap;

/// A parsed s-expression.
#[derive(Debug, Clone, PartialEq)]
enum Node {
    Atom(String, usize),
    List(Vec<Node>),
}

impl Node {
    fn atom(&self) -> Option<&str> {
        match self {
            Node::Atom(a, _) => Some(a),
            Node::List(_) => None,
        }
    }
}

fn parse(text: &str) -> Vec<Node> {
    let mut stack: Vec<Vec<Node>> = vec![Vec::new()];
    let mut line = 1;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => line += 1,
            c if c.is_whitespace() => {}
            ';' if chars.peek() == Some(&';') => while chars.next_if(|c| *c != '\n').is_some() {},
            '(' => stack.push(Vec::new()),
            ')' => {
                if stack.len() > 1 {
                    let list = stack.pop().unwrap_or_default();
                    stack.last_mut().expect("root").push(Node::List(list));
                }
            }
            c => {
                let mut atom = String::from(c);
                while let Some(n) = chars.next_if(|n| !n.is_whitespace() && *n != '(' && *n != ')')
                {
                    atom.push(n);
                }
                stack.last_mut().expect("root").push(Node::Atom(atom, line));
            }
        }
    }
    stack.swap_remove(0)
}

/// What a key does in a layer.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Falls through to the layer below (or the key itself when nothing is below).
    Transparent,
    /// Does nothing.
    Blocked,
    /// Emits keys: `outputs` are kanata key names, modifiers first.
    Keys { label: String, outputs: Vec<String> },
    /// Switches to a layer for good.
    Switch(String),
    /// Switches to a layer while held.
    Hold(String),
    /// Anything else (macros, tap-hold, ...), named by its head.
    Other(String),
}

#[derive(Debug, Default)]
pub struct Layer {
    pub name: String,
    keys: HashMap<String, Action>,
    default: Option<Action>,
}

#[derive(Debug, Default)]
pub struct Config {
    /// Physical rows from `defsrc`, one per source line.
    pub rows: Vec<Vec<String>>,
    pub layers: Vec<Layer>,
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, String> {
        let nodes = parse(text);
        let mut cfg = Config::default();
        let mut aliases: HashMap<String, Node> = HashMap::new();
        collect_aliases(&nodes, &mut aliases);
        for node in &nodes {
            let Node::List(items) = node else { continue };
            match items.first().and_then(Node::atom) {
                Some("defsrc") => cfg.rows = rows_of(&items[1..]),
                Some("deflayermap") => cfg.layers.push(layer_of(&items[1..], &aliases)?),
                _ => {}
            }
        }
        if cfg.rows.is_empty() {
            return Err("no defsrc found".into());
        }
        if cfg.layers.is_empty() {
            return Err("no deflayermap found".into());
        }
        Ok(cfg)
    }

    pub fn layer(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name == name)
    }

    /// Layers entered by holding a key: their transparent keys fall through to the base layer.
    pub fn is_overlay(&self, name: &str) -> bool {
        self.layers
            .iter()
            .flat_map(|l| l.keys.values().chain(l.default.iter()))
            .any(|a| matches!(a, Action::Hold(n) if n == name))
    }

    /// What `key` does in `layer` when `base` is the layer below it.
    pub fn action(&self, layer: &str, base: &str, key: &str) -> Action {
        let own = self.layer(layer).map(|l| l.action(key));
        match own {
            Some(Action::Transparent) if layer != base => self.action(base, base, key),
            Some(Action::Transparent) | None => pass(key),
            Some(a) => a,
        }
    }
}

impl Layer {
    fn action(&self, key: &str) -> Action {
        self.keys.get(key).or(self.default.as_ref()).cloned().unwrap_or(Action::Transparent)
    }
}

/// A key that does what it says.
fn pass(key: &str) -> Action {
    Action::Keys { label: key.to_owned(), outputs: vec![key.to_owned()] }
}

fn collect_aliases(nodes: &[Node], out: &mut HashMap<String, Node>) {
    for node in nodes {
        let Node::List(items) = node else { continue };
        match items.first().and_then(Node::atom) {
            Some("defalias") => {
                for pair in items[1..].chunks(2) {
                    if let [Node::Atom(name, _), body] = pair {
                        out.insert(name.clone(), body.clone());
                    }
                }
            }
            // `(platform (macos) ...)`: later definitions win, so win/linux ones follow macOS ones.
            Some("platform") => collect_aliases(&items[2..], out),
            _ => {}
        }
    }
}

fn rows_of(items: &[Node]) -> Vec<Vec<String>> {
    let mut rows: Vec<(usize, Vec<String>)> = Vec::new();
    for item in items {
        if let Node::Atom(key, line) = item {
            match rows.last_mut() {
                Some((l, row)) if l == line => row.push(key.clone()),
                _ => rows.push((*line, vec![key.clone()])),
            }
        }
    }
    rows.into_iter().map(|(_, r)| r).collect()
}

fn layer_of(items: &[Node], aliases: &HashMap<String, Node>) -> Result<Layer, String> {
    let name = match items.first() {
        Some(Node::Atom(n, _)) => n.clone(),
        Some(Node::List(l)) => {
            l.first().and_then(Node::atom).map(str::to_owned).unwrap_or_default()
        }
        None => String::new(),
    };
    if name.is_empty() {
        return Err("a deflayermap has no name".into());
    }
    let mut layer = Layer { name, ..Layer::default() };
    for pair in items[1..].chunks(2) {
        let [Node::Atom(key, _), body] = pair else { continue };
        let action = action_of(body, aliases);
        if key == "___" {
            layer.default = Some(action);
        } else {
            layer.keys.insert(key.clone(), action);
        }
    }
    Ok(layer)
}

fn action_of(node: &Node, aliases: &HashMap<String, Node>) -> Action {
    match node {
        Node::Atom(a, _) => match a.as_str() {
            "_" => Action::Transparent,
            "XX" => Action::Blocked,
            _ => match a.strip_prefix('@') {
                Some(name) => match aliases.get(name).map(|n| action_of(n, aliases)) {
                    Some(Action::Keys { outputs, .. }) => {
                        Action::Keys { label: name.to_owned(), outputs }
                    }
                    Some(Action::Transparent | Action::Blocked) | None => {
                        Action::Other(name.to_owned())
                    }
                    Some(other) => other,
                },
                None => keys_of(a),
            },
        },
        Node::List(items) => {
            match (items.first().and_then(Node::atom), items.get(1).and_then(Node::atom)) {
                (Some("layer-switch"), Some(l)) => Action::Switch(l.to_owned()),
                (Some("layer-while-held"), Some(l)) => Action::Hold(l.to_owned()),
                (Some(head), _) => Action::Other(head.to_owned()),
                (None, _) => Action::Other("?".into()),
            }
        }
    }
}

/// `a`, `lctl`, `S-1`, `C-x`: a key with optional modifier prefixes.
fn keys_of(atom: &str) -> Action {
    let mut mods = Vec::new();
    let mut rest = atom;
    while let Some((m, tail)) = rest.split_once('-')
        && !tail.is_empty()
        && let Some(name) = prefix_mod(m)
    {
        mods.push(name.to_owned());
        rest = tail;
    }
    let label = match (mods.as_slice(), shifted(rest)) {
        ([only], Some(s)) if only == "lsft" => s,
        _ => short(atom),
    };
    let mut outputs = mods;
    outputs.push(rest.to_owned());
    Action::Keys { label, outputs }
}

fn prefix_mod(prefix: &str) -> Option<&'static str> {
    match prefix {
        "C" | "LC" => Some("lctl"),
        "RC" => Some("rctl"),
        "S" | "LS" => Some("lsft"),
        "RS" => Some("rsft"),
        "A" | "LA" => Some("lalt"),
        "RA" => Some("ralt"),
        "M" | "LM" => Some("lmet"),
        "RM" => Some("rmet"),
        _ => None,
    }
}

/// What a US keyboard types for Shift plus this key.
fn shifted(key: &str) -> Option<String> {
    let mapped = match key {
        "1" => "!",
        "2" => "@",
        "3" => "#",
        "4" => "$",
        "5" => "%",
        "6" => "^",
        "7" => "&",
        "8" => "*",
        "9" => "(",
        "0" => ")",
        "-" => "_",
        "=" => "+",
        "[" => "{",
        "]" => "}",
        "\\" => "|",
        ";" => ":",
        "'" => "\"",
        "," => "<",
        "." => ">",
        "/" => "?",
        "grv" | "`" => "~",
        k if k.len() == 1 && k.chars().all(|c| c.is_ascii_lowercase()) => {
            return Some(k.to_uppercase());
        }
        _ => return None,
    };
    Some(mapped.to_owned())
}

/// A short, readable name for a key.
pub fn short(key: &str) -> String {
    match key {
        "lctl" | "rctl" => "ctl",
        "lsft" | "rsft" => "sft",
        "lalt" | "ralt" => "alt",
        "lmet" | "rmet" => "met",
        "up" => "↑",
        "down" => "↓",
        "left" => "←",
        "right" => "→",
        "grv" => "`",
        k => k,
    }
    .to_owned()
}

#[cfg(test)]
mod tests;
