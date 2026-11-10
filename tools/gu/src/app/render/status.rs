//! The status line.

use super::super::*;

impl App {
    pub(in crate::app) fn render_status(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let line = match &self.mode {
            Mode::Prompt(kind, input) => {
                let label = match kind {
                    PromptKind::Search => "search: ",
                    PromptKind::NewBranch => "new branch: ",
                    PromptKind::NewStash => "stash message: ",
                    PromptKind::RenameBranch(_) => "rename to: ",
                    PromptKind::RenameTag(_) => "rename tag to: ",
                    PromptKind::RenameRemote(_) => "rename remote to: ",
                    PromptKind::RenameRemoteBranch(_) => "rename on the remote to: ",
                    PromptKind::RenameFolder { .. } => "rename folder to: ",
                    PromptKind::BranchAt(_) => "new branch here: ",
                    PromptKind::TagAt(_) => "tag name: ",
                    PromptKind::Reword(_) => "message: ",
                    PromptKind::Amend(_) => "amend with message: ",
                    PromptKind::SwitchTo => "switch to (branch, tag or commit): ",
                    PromptKind::AddSubmodule => "submodule url [path]: ",
                };
                ui::prompt::cursor(buf, area, label, input.visual_cursor());
                if matches!(kind, PromptKind::SwitchTo) {
                    self.suggest.render(buf, area, theme);
                }
                ui::prompt::line(theme, label, input.value())
            }
            Mode::Confirm(cf) => {
                let q = match cf {
                    Confirm::DeleteBranch { name, force } => {
                        format!(
                            "{} branch {name}? (d again or y)",
                            if *force { "Force-delete" } else { "Delete" }
                        )
                    }
                    Confirm::Merge(name) => {
                        format!("Merge {name} into {}? (y/n)", self.files.branch())
                    }
                    Confirm::DropStash(i) => format!("Drop stash@{{{i}}}? (d again or y)"),
                    Confirm::Rebase(name) => {
                        format!("Rebase {} onto {name}? (y/n)", self.files.branch())
                    }
                    Confirm::Revert(_) => "Revert this commit? (y/n)".to_string(),
                    Confirm::Reset { .. } => {
                        "Reset hard to this commit and lose uncommitted changes? (y/n)".to_string()
                    }
                    Confirm::ForcePush => {
                        format!("Force-push {} (with lease)? (y/n)", self.files.branch())
                    }
                    Confirm::DeleteTag(name) => format!("Delete tag {name}? (d again or y)"),
                    Confirm::RemoveSubmodule(path) => {
                        format!("Remove submodule {path} and delete its folder? (d again or y)")
                    }
                    Confirm::DeleteRemote(name) => {
                        format!("Delete {name} on the remote? (d again or y)")
                    }
                };
                Line::from(Span::styled(q, Style::new().fg(c.warning.into())))
            }
            Mode::Popup(_) => Line::default(),
            Mode::Normal if self.job.is_some() => Line::from(Span::styled(
                format!(
                    "{} {}",
                    ui::anim::spinner(),
                    self.job.as_ref().map(|j| j.label.as_str()).unwrap_or("")
                ),
                Style::new().fg(c.accent.into()),
            )),
            Mode::Normal => match &self.message {
                Some((m, err)) => Line::from(Span::styled(
                    m.clone(),
                    Style::new().fg(if *err { c.error } else { c.accent }.into()),
                )),
                None => match self.operation_name() {
                    Some(op) => Line::from(vec![
                        Span::styled(
                            format!("{op} "),
                            Style::new().fg(c.warning.into()).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            "resolve the conflicts and stage them, then space o c continues, space o a aborts",
                            Style::new().fg(c.muted.into()),
                        ),
                    ]),
                    None => Line::from(Span::styled(self.hints(), Style::new().fg(c.muted.into()))),
                },
            },
        };
        Paragraph::new(line)
            .render(Rect { x: area.x + 1, width: area.width.saturating_sub(2), ..area }, buf);
    }

    pub(in crate::app) fn hints(&self) -> &'static str {
        if self.screen == Screen::Commits {
            if self.commit_files.is_some() {
                return "j k move  l h fold  ctrl-d/u scroll diff  } { hunks  ) ( files  esc back to the commits";
            }
            return "/ search  n N next  f filter  F clear  v view  l files  y copy hash  enter checkout  C cherry-pick  [ ] tabs  ? keys";
        }
        match self.focus {
            Focus::Files => {
                "s stage/unstage  a all  d discard  t tree/list  c commit  enter edit  [ ] tabs  space space commands  ? keys"
            }
            Focus::Branches => {
                "enter checkout  n new  d delete  M merge  B rebase  tab stashes  space r fetch/pull/push  [ ] tabs  ? keys"
            }
            Focus::Commits => {
                "enter checkout  m compare  M vs work tree  C cherry-pick  R revert  r reword  A amend  b branch  t tag  space z reset  space space commands  ? keys"
            }
            Focus::Stashes => "n new  a apply  p pop  d drop  tab refs  [ ] tabs  ? keys",
            Focus::Submodules => {
                "enter open  u update  U all  r remote tip  s sync  n add  d remove  space U back up  [ ] tabs  ? keys"
            }
        }
    }
}
