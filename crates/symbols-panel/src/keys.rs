//! The symbol explorer's actions and default keys (see `docs/usage-design.md`).

use ui::keys::{Action, Group};

macro_rules! b {
    ($id:literal, $name:literal, $desc:literal, [$($k:literal),*]) => {
        Action { id: $id, name: $name, desc: $desc, basic: true, keys: &[$($k),*] }
    };
}

macro_rules! a {
    ($id:literal, $name:literal, $desc:literal, [$($k:literal),*]) => {
        Action { id: $id, name: $name, desc: $desc, basic: false, keys: &[$($k),*] }
    };
}

pub const ACTIONS: &[Action] = &[
    b!("nav.up", "up", "move the cursor up", ["k", "up", "ctrl-p"]),
    b!("nav.down", "down", "move the cursor down", ["j", "down", "ctrl-n"]),
    b!("nav.up5", "up 5", "move the cursor up five rows", ["ctrl-k"]),
    b!("nav.down5", "down 5", "move the cursor down five rows", ["ctrl-j"]),
    b!("nav.page_up", "page up", "move a page up", ["pageup"]),
    b!("nav.page_down", "page down", "move a page down", ["pagedown"]),
    b!("nav.top", "top", "go to the first row", ["g", "home"]),
    b!("nav.bottom", "bottom", "go to the last row", ["G", "end"]),
    b!("tree.out", "collapse", "collapse the symbol, or go to its parent", ["h", "left"]),
    b!("tree.in", "expand", "expand the symbol", ["l", "right"]),
    b!("item.open", "go to", "jump to the symbol in the editor (on one with members: also expand it)", ["enter"]),
    a!("tree.fold_all", "fold all", "collapse every symbol with members", ["leader z c"]),
    a!("tree.unfold_all", "unfold all", "expand every symbol", ["leader z o"]),
    a!("filter.start", "filter", "show only the symbols whose name contains the text", ["/", "leader f"]),
    b!("palette", "commands", "search every action by name and run it", ["leader space"]),
    b!("help", "keys", "list every action with its keys", ["?"]),
    b!("quit", "back", "give the keyboard back to the editor", ["q", "esc", "ctrl-c"]),
];

pub const GROUPS: &[Group] = &[Group { prefix: "leader z", name: "fold" }];

pub fn owns(id: &str) -> bool {
    ACTIONS.iter().any(|a| a.id == id)
}

#[cfg(test)]
mod tests;
