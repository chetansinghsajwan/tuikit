//! Actions on the row under the cursor or the marked rows: open, mark, create, rename, delete,
//! copy, cut, paste, open with and bulk rename.

use super::*;
use ui::keys::{Action as Key, action, basic};

/// The actions of this domain with their default keys.
pub(super) const ACTIONS: &[Key] = &[
    basic("item.open", "open", "open the file, or expand the folder", &["enter", "l", "right"]),
    basic("item.mark", "mark", "mark or unmark the row for a multi-row action", &["tab"]),
    action("item.new", "new", "create a file (a trailing / makes a folder)", &["a", "leader n"]),
    action("item.rename", "rename", "rename the row", &["r", "leader r"]),
    action(
        "item.delete",
        "delete",
        "move the marked rows to the trash (asks first)",
        &["d", "leader d"],
    ),
    action(
        "item.delete_forever",
        "delete forever",
        "delete the marked rows permanently (asks first)",
        &["D"],
    ),
    action("item.yank", "copy", "copy the marked rows", &["y", "leader y"]),
    action("item.cut", "cut", "cut the marked rows", &["x", "leader x"]),
    action("item.paste", "paste", "paste into the folder under the cursor", &["p", "leader p"]),
    action("item.open_with", "open with", "open the row with a command you type", &["leader o"]),
    action(
        "item.bulk_rename",
        "bulk rename",
        "rename the marked rows together in your editor",
        &["leader B"],
    ),
];

impl Panel {
    /// Runs one of this domain's actions by id.
    pub(super) fn run_item(&mut self, id: &str) -> Action {
        match id {
            "item.open" => {
                if let Some(row) = self.rows.get(self.cursor) {
                    if row.is_dir {
                        self.toggle(Some(true));
                    } else {
                        return Action::Open(row.path.clone());
                    }
                }
            }
            "item.mark" => {
                if let Some(path) = self.current().map(Path::to_path_buf) {
                    if !self.marked.remove(&path) {
                        self.marked.insert(path);
                    }
                    self.move_by(1);
                }
            }
            "item.new" => {
                self.mode = Mode::Prompt(PromptKind::Create(self.target_dir()), Input::default())
            }
            "item.rename" => {
                if let Some(row) = self.rows.get(self.cursor) {
                    self.mode = Mode::Prompt(
                        PromptKind::Rename(row.path.clone()),
                        Input::new(row.name.clone()),
                    );
                }
            }
            "item.delete" | "item.delete_forever" => {
                let targets = self.targets();
                if !targets.is_empty() {
                    self.mode = Mode::Confirm(
                        if id == "item.delete" { ConfirmKind::Trash } else { ConfirmKind::Delete },
                        targets,
                    );
                }
            }
            "item.yank" | "item.cut" => {
                let paths = self.targets();
                if !paths.is_empty() {
                    let (n, cut) = (paths.len(), id == "item.cut");
                    self.clip = Some(Clip { paths, cut });
                    self.marked.clear();
                    self.say(
                        format!(
                            "{} {n} item{}",
                            if cut { "cut" } else { "yanked" },
                            if n == 1 { "" } else { "s" }
                        ),
                        false,
                    );
                }
            }
            "item.paste" => return self.paste(),
            "item.open_with" => {
                if let Some(row) = self.rows.get(self.cursor) {
                    self.mode =
                        Mode::Prompt(PromptKind::OpenWith(row.path.clone()), Input::default());
                }
            }
            "item.bulk_rename" => {
                let t = self.targets();
                if !t.is_empty() {
                    return Action::BulkRename(t);
                }
            }
            _ => {}
        }
        Action::None
    }
}
