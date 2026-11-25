//! Branches, tags and remote branches: their actions, prompts and questions.

use super::*;

impl App {
    /// The actions that act on the highlighted branch, tag or remote branch.
    pub(super) fn branch_action(&mut self, id: &str) {
        // Renaming a folder renames everything under it (a remote folder renames the remote itself).
        if id == "branch.rename" {
            if let Some(RefRow::Folder { key, depth, .. }) =
                self.ref_rows.get(self.lists[0].cursor()).cloned()
            {
                let (section, path) = key.split_once('/').unwrap_or(("", &key));
                match section {
                    "b" => {
                        self.mode = Mode::Prompt(
                            PromptKind::RenameFolder {
                                kind: RefKind::Branch,
                                prefix: path.to_string(),
                            },
                            Input::new(path.to_string()),
                        )
                    }
                    "t" => {
                        self.mode = Mode::Prompt(
                            PromptKind::RenameFolder {
                                kind: RefKind::Tag,
                                prefix: path.to_string(),
                            },
                            Input::new(path.to_string()),
                        )
                    }
                    _ if depth == 1 => {
                        self.mode = Mode::Prompt(
                            PromptKind::RenameRemote(path.to_string()),
                            Input::new(path.to_string()),
                        )
                    }
                    _ => self.show_message(
                        "rename a remote branch itself, a remote folder is only part of its name",
                        true,
                    ),
                }
                return;
            }
            if matches!(self.ref_rows.get(self.lists[0].cursor()), Some(RefRow::Section { .. })) {
                self.show_message("pick a ref or a folder to rename", true);
                return;
            }
        }
        // On a section or folder the Enter key folds or unfolds it.
        if let Some(RefRow::Section { key, .. } | RefRow::Folder { key, .. }) =
            self.ref_rows.get(self.lists[0].cursor())
            && id == "branch.checkout"
        {
            let key = key.clone();
            if !self.folded.remove(&key) {
                self.folded.insert(key);
            }
            let keep = self.ref_row_id(self.lists[0].cursor());
            self.build_ref_rows(keep);
            return;
        }
        let Some(b) = self.current_ref().cloned() else {
            if id == "branch.new" {
                self.mode = Mode::Prompt(PromptKind::NewBranch, Input::default());
            }
            return;
        };
        match id {
            "branch.checkout" => {
                if b.is_head {
                    self.show_message(format!("already on {}", b.name), false);
                } else {
                    let (name, kind) = (b.name.clone(), b.kind);
                    self.run(format!("switched to {}", b.name), move |r| {
                        r.checkout_ref(&name, kind)
                    });
                }
            }
            "branch.new" => self.mode = Mode::Prompt(PromptKind::NewBranch, Input::default()),
            "branch.delete" | "branch.force_delete" => match b.kind {
                RefKind::Tag => self.mode = Mode::Confirm(Confirm::DeleteTag(b.name)),
                RefKind::Remote => self.mode = Mode::Confirm(Confirm::DeleteRemote(b.name)),
                RefKind::Branch if b.is_head => {
                    self.show_message("the checked-out branch cannot be deleted", true)
                }
                RefKind::Branch => {
                    self.mode = Mode::Confirm(Confirm::DeleteBranch {
                        name: b.name,
                        force: id == "branch.force_delete",
                    })
                }
            },
            "branch.rebase" => {
                if b.is_head {
                    self.show_message("cannot rebase a branch onto itself", true);
                } else {
                    self.mode = Mode::Confirm(Confirm::Rebase(b.name));
                }
            }
            "branch.rename" => {
                let kind = match b.kind {
                    RefKind::Branch => PromptKind::RenameBranch(b.name.clone()),
                    RefKind::Tag => PromptKind::RenameTag(b.name.clone()),
                    RefKind::Remote => PromptKind::RenameRemoteBranch(b.name.clone()),
                };
                // A remote branch is renamed without its remote (`topic`, not `origin/topic`).
                let shown = if b.kind == RefKind::Remote {
                    b.name.split_once('/').map(|(_, n)| n.to_string()).unwrap_or_default()
                } else {
                    b.name
                };
                self.mode = Mode::Prompt(kind, Input::new(shown));
            }
            "branch.merge" => {
                if b.is_head {
                    self.show_message("cannot merge a branch into itself", true);
                } else {
                    self.mode = Mode::Confirm(Confirm::Merge(b.name));
                }
            }
            _ => {}
        }
    }

    /// Run a finished branch, tag or remote prompt.
    pub(super) fn branch_submit(&mut self, kind: PromptKind, value: String) {
        match kind {
            PromptKind::NewBranch => {
                let v = value.clone();
                self.run(format!("created {value}"), move |r| r.create_branch(&v));
            }
            PromptKind::RenameBranch(old) => {
                let v = value.clone();
                self.run(format!("renamed {old} to {value}"), move |r| r.rename_branch(&old, &v));
            }
            PromptKind::RenameTag(old) => {
                let v = value.clone();
                self.run(format!("renamed {old} to {value}"), move |r| r.rename_tag(&old, &v));
            }
            PromptKind::RenameRemote(old) => {
                let v = value.clone();
                self.run(format!("renamed remote {old} to {value}"), move |r| {
                    r.rename_remote(&old, &v)
                });
            }
            PromptKind::RenameRemoteBranch(old) => {
                let v = value.clone();
                let done = format!("renamed {old} to {value} on the remote");
                self.remote("renaming on the remote", &done, move |r| {
                    r.rename_remote_branch(&old, &v)
                });
            }
            PromptKind::RenameFolder { kind, prefix } => {
                let value = value.trim_end_matches('/').to_string();
                let pairs: Vec<(String, String)> = self
                    .branches
                    .iter()
                    .filter(|r| r.kind == kind && r.name.starts_with(&format!("{prefix}/")))
                    .map(|r| (r.name.clone(), format!("{value}{}", &r.name[prefix.len()..])))
                    .collect();
                let n = pairs.len();
                // The names are checked and the renames are all or none (`Repo::rename_many`).
                if value.is_empty() || value.starts_with('/') || value.contains("//") {
                    self.show_message("the folder name is empty or has an empty part", true);
                } else {
                    self.run(
                        format!("renamed {n} ref{} to {value}/", if n == 1 { "" } else { "s" }),
                        move |r| r.rename_many(kind, &pairs),
                    );
                }
            }
            _ => {}
        }
    }

    /// Run a confirmed branch, tag or remote change.
    pub(super) fn branch_confirmed(&mut self, c: Confirm) {
        match c {
            Confirm::DeleteBranch { name, force } => {
                let n = name.clone();
                self.run(format!("deleted {name}"), move |r| r.delete_branch(&n, force));
            }
            Confirm::Merge(name) => {
                let n = name.clone();
                self.run(format!("merged {name}"), move |r| r.merge(&n));
            }
            Confirm::Rebase(name) => {
                let n = name.clone();
                self.run(format!("rebased onto {name}"), move |r| r.rebase(&n));
            }
            Confirm::DeleteTag(name) => {
                let n = name.clone();
                self.run(format!("deleted tag {name}"), move |r| r.delete_tag(&n));
            }
            Confirm::DeleteRemote(name) => {
                let n = name.clone();
                self.remote("deleting", "deleted", move |r| r.delete_remote_branch(&n));
            }
            _ => {}
        }
    }

    /// The `switch.to` prompt, which checks out a ref or commit by name.
    pub(super) fn switch_action(&mut self, id: &str) -> bool {
        match id {
            "switch.to" => {
                let names = self.branches.iter().map(|b| b.name.clone()).collect();
                self.suggest = ui::prompt::Suggest::new(names);
                self.mode = Mode::Prompt(PromptKind::SwitchTo, Input::default());
            }
            _ => return false,
        }
        true
    }

    /// Check out what was typed: a ref by its name, the option chosen from the list, or a commit.
    pub(super) fn switch_to(&mut self, typed: String) {
        let typed = typed.trim().to_string();
        let by = |name: &str| {
            self.branches
                .iter()
                .find(|b| b.name == name)
                .map(|b| (b.name.clone(), b.kind, b.is_head))
        };
        let pick = by(&typed).or_else(|| self.suggest.selected().and_then(|s| by(&s)));
        match pick {
            Some((name, _, true)) => self.show_message(format!("already on {name}"), false),
            Some((name, kind, false)) => {
                self.run(format!("switched to {name}"), move |r| r.checkout_ref(&name, kind))
            }
            None if typed.is_empty() => {}
            None => match self.repo.commit(&typed) {
                Ok(k) => {
                    let (oid, short) = (k.oid.clone(), k.short.clone());
                    self.run(format!("checked out {short}"), move |r| r.checkout_commit(&oid));
                }
                Err(_) => {
                    self.show_message(format!("no branch, tag or commit named {typed}"), true)
                }
            },
        }
    }
}
