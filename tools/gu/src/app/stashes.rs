//! Stashes: their actions, prompts and questions.

use super::*;

impl App {
    /// The actions that act on the highlighted stash.
    pub(super) fn stash_action(&mut self, id: &str) {
        let cur = self.stashes.get(self.lists[2].cursor()).map(|s| s.index);
        match (id, cur) {
            ("stash.new", _) => self.mode = Mode::Prompt(PromptKind::NewStash, Input::default()),
            ("stash.apply", Some(i)) => {
                self.run(format!("applied stash@{{{i}}}"), move |r| r.stash_apply(i))
            }
            ("stash.pop", Some(i)) => {
                self.run(format!("popped stash@{{{i}}}"), move |r| r.stash_pop(i))
            }
            ("stash.drop", Some(i)) => self.mode = Mode::Confirm(Confirm::DropStash(i)),
            _ => {}
        }
    }

    /// Run a finished stash prompt.
    pub(super) fn stash_submit(&mut self, kind: PromptKind, value: String) {
        if let PromptKind::NewStash = kind {
            let v = value.clone();
            self.run("stashed the changes".to_string(), move |r| r.stash_push(&v));
        }
    }

    /// Run a confirmed stash change.
    pub(super) fn stash_confirmed(&mut self, c: Confirm) {
        if let Confirm::DropStash(i) = c {
            self.run(format!("dropped stash@{{{i}}}"), move |r| r.stash_drop(i))
        }
    }
}
