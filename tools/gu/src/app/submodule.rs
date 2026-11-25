//! The actions of the Submodules pane.

use super::*;

impl App {
    /// The actions that act on the highlighted submodule.
    pub(super) fn submodule_action(&mut self, id: &str) {
        let cur = self.submodules.get(self.lists[3].cursor).cloned();
        match (id, cur) {
            ("submodule.add", _) => {
                self.mode = Mode::Prompt(PromptKind::AddSubmodule, Input::default())
            }
            ("submodule.update_all", _) => {
                self.remote("updating", "updated every submodule", |r| r.submodule_update_all())
            }
            ("submodule.sync", _) => {
                self.run("synced the urls".to_string(), |r| r.submodule_sync())
            }
            ("submodule.open", Some(s)) => self.open_submodule(&s.path),
            ("submodule.update", Some(s)) => {
                let done = format!("updated {}", s.path);
                self.remote("updating", &done, move |r| r.submodule_update(&s.path));
            }
            ("submodule.update_remote", Some(s)) => {
                let done = format!("{} is at the remote tip", s.path);
                self.remote("updating", &done, move |r| r.submodule_update_remote(&s.path));
            }
            ("submodule.remove", Some(s)) => {
                self.mode = Mode::Confirm(Confirm::RemoveSubmodule(s.path))
            }
            _ => {}
        }
    }

    /// Show the submodule at `path` as the repository on screen, remembering this one.
    fn open_submodule(&mut self, path: &str) {
        let dir = self.root.join(path);
        // A submodule that is not cloned has an empty folder: clone it first.
        if Repo::discover(&dir)
            .is_none_or(|r| r.root() != dir.canonicalize().unwrap_or(dir.clone()))
        {
            self.show_message(format!("{path} is not cloned: u updates it first"), true);
            return;
        }
        let Some(mut app) = App::new(&dir) else {
            self.show_message(format!("cannot open {path}"), true);
            return;
        };
        app.parents = std::mem::take(&mut self.parents);
        app.parents.push(self.root.clone());
        app.tab = Tab::Refs;
        app.focus = Focus::Branches;
        *self = app;
    }

    /// Go back to the repository the current one was opened from.
    pub(super) fn repo_up(&mut self) {
        let Some(parent) = self.parents.pop() else {
            self.show_message("this is the top repository", false);
            return;
        };
        let parents = std::mem::take(&mut self.parents);
        match App::new(&parent) {
            Some(mut app) => {
                app.parents = parents;
                app.tab = Tab::Refs;
                app.focus = Focus::Submodules;
                *self = app;
            }
            None => {
                self.parents = parents;
                self.parents.push(parent);
                self.show_message("the parent repository is gone", true);
            }
        }
    }

    /// Run a finished submodule prompt.
    pub(super) fn submodule_submit(&mut self, kind: PromptKind, value: String) {
        if let PromptKind::AddSubmodule = kind {
            let mut parts = value.split_whitespace();
            let Some(url) = parts.next().map(str::to_string) else { return };
            // Without a path, the last part of the URL names the folder, as `git submodule add` does.
            let path = parts.next().map(str::to_string).unwrap_or_else(|| {
                let tail = url.trim_end_matches('/').rsplit(['/', ':']).next();
                tail.unwrap_or("").trim_end_matches(".git").to_string()
            });
            let done = format!("added {path}");
            self.remote("adding the submodule", &done, move |r| r.submodule_add(&url, &path));
        }
    }

    /// Run a confirmed submodule change.
    pub(super) fn submodule_confirmed(&mut self, c: Confirm) {
        if let Confirm::RemoveSubmodule(path) = c {
            let p = path.clone();
            self.run(format!("removed {path}"), move |r| r.submodule_remove(&p));
        }
    }
}
