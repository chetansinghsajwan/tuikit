//! The input modes: a prompt, a question or a popup holds the keys. Each feature runs its own answers.

use super::*;

impl App {
    /// Handles an event the screen claimed (see `captures`): a prompt, a question, a popup or the
    /// Files panel's own prompts. Tells the host what to do next.
    pub(super) fn typing(&mut self, ev: &Event) -> Outcome {
        let Some(key) = ui::keys::pressed(ev) else { return Outcome::Continue };
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Prompt(kind, input) => {
                self.prompt_key(kind, input, ev, key);
                return Outcome::Continue;
            }
            Mode::Confirm(c) => {
                self.confirm_key(c, key);
                return Outcome::Continue;
            }
            Mode::Popup(p) => {
                self.popup_key(p, key);
                return Outcome::Continue;
            }
            Mode::Normal => {}
        }
        if self.focus == Focus::Files && self.files.captures_input(ev) {
            let action = self.files.handle_captured(ev);
            return self.files_action(action);
        }
        Outcome::Continue
    }

    /// Whether the screen takes this event itself: a prompt, a question or a popup is open, or the
    /// Files panel is asking something.
    pub(super) fn takes(&self, ev: &Event) -> bool {
        !matches!(self.mode, Mode::Normal)
            || (self.focus == Focus::Files && self.files.captures_input(ev))
    }

    /// A key in a prompt: edit the line, and on enter run what the prompt was asking for.
    pub(super) fn prompt_key(
        &mut self,
        kind: PromptKind,
        mut input: Input,
        ev: &Event,
        key: &KeyEvent,
    ) {
        // Filter prompts offer options: up and down choose, tab fills the line in.
        if matches!(kind, PromptKind::SwitchTo) {
            let ctrl = key.modifiers.contains(ratatui::crossterm::event::KeyModifiers::CONTROL);
            let step = match key.code {
                KeyCode::Down | KeyCode::Tab => Some(1),
                KeyCode::Up | KeyCode::BackTab => Some(-1),
                KeyCode::Char('n') if ctrl => Some(1),
                KeyCode::Char('p') if ctrl => Some(-1),
                _ => None,
            };
            if let Some(d) = step {
                self.suggest.set_typed_text(input.value());
                if matches!(key.code, KeyCode::Tab) {
                    if let Some(s) = self.suggest.selected() {
                        input = Input::new(s);
                    }
                } else {
                    self.suggest.move_selection(d);
                }
                self.mode = Mode::Prompt(kind, input);
                return;
            }
        }
        match ui::prompt::typed(&mut input, ev) {
            Typed::Cancel => {}
            Typed::Submit(value) => match kind {
                PromptKind::Search => self.search_submit(kind, value),
                PromptKind::NewBranch
                | PromptKind::RenameBranch(_)
                | PromptKind::RenameTag(_)
                | PromptKind::RenameRemote(_)
                | PromptKind::RenameRemoteBranch(_)
                | PromptKind::RenameFolder { .. } => self.branch_submit(kind, value),
                PromptKind::SwitchTo => self.switch_to(value),
                PromptKind::BranchAt(_)
                | PromptKind::TagAt(_)
                | PromptKind::Reword(_)
                | PromptKind::Amend(_) => self.commit_submit(kind, value),
                PromptKind::NewStash => self.stash_submit(kind, value),
                PromptKind::AddSubmodule => self.submodule_submit(kind, value),
            },
            Typed::Editing => {
                self.suggest.set_typed_text(input.value());
                self.mode = Mode::Prompt(kind, input);
            }
        }
    }

    /// A key at a yes or no question; destructive ones need `d` again or `y`.
    pub(super) fn confirm_key(&mut self, c: Confirm, key: &KeyEvent) {
        let again = matches!(
            c,
            Confirm::DeleteBranch { .. }
                | Confirm::DeleteTag(_)
                | Confirm::DeleteRemote(_)
                | Confirm::DropStash(_)
                | Confirm::RemoveSubmodule(_)
        );
        if !(matches!(key.code, KeyCode::Char('y' | 'Y') | KeyCode::Enter)
            || (again && matches!(key.code, KeyCode::Char('d' | 'D'))))
        {
            return;
        }
        match c {
            Confirm::DeleteBranch { .. }
            | Confirm::Merge(_)
            | Confirm::Rebase(_)
            | Confirm::DeleteTag(_)
            | Confirm::DeleteRemote(_) => self.branch_confirmed(c),
            Confirm::Revert(_) | Confirm::Reset { .. } => self.commit_confirmed(c),
            Confirm::DropStash(_) => self.stash_confirmed(c),
            Confirm::ForcePush => self.remote_confirmed(c),
            Confirm::RemoveSubmodule(_) => self.submodule_confirmed(c),
        }
    }
}
