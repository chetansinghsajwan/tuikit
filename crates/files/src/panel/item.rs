//! Actions on the row under the cursor or the marked rows: open, mark, create, rename, delete,
//! copy, cut, paste, open with and bulk rename.

use super::*;

impl Panel {
    /// Runs one of this domain's actions by id.
    pub(super) fn run_item_action(&mut self, id: &str) -> Outcome {
        match id {
            "item.open" => {
                if let Some(row) = self.rows.get(self.cursor) {
                    if row.is_dir {
                        self.toggle(Some(true));
                    } else {
                        return Outcome::Open(row.path.clone());
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
            "item.create" => {
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
            "item.copy" | "item.cut" => {
                let paths = self.targets();
                if !paths.is_empty() {
                    let (n, cut) = (paths.len(), id == "item.cut");
                    self.clip = Some(Clip { paths, cut });
                    self.marked.clear();
                    self.show_message(
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
                    return Outcome::BulkRename(t);
                }
            }
            _ => {}
        }
        Outcome::None
    }
}
