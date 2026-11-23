//! How the commit list and the preview are drawn: columns, graph, detail and the diff view.

use super::*;

impl App {
    /// The actions that change what the list shows.
    pub(super) fn view_action(&mut self, id: &str) -> bool {
        match id {
            "view.detail" => self.detail = !self.detail,
            "view.graph" => {
                self.graph = !self.graph;
                self.reload_history();
            }
            "view.hash" => self.cols.show_hash = !self.cols.show_hash,
            "view.author" => self.cols.show_author = !self.cols.show_author,
            "view.author_name" => self.cols.show_author_name = !self.cols.show_author_name,
            "view.refs" => self.cols.show_refs = !self.cols.show_refs,
            "view.subject" => self.cols.show_subject = !self.cols.show_subject,
            "view.rel_time" => self.cols.show_relative_time = !self.cols.show_relative_time,
            "view.abs_time" => self.cols.show_absolute_time = !self.cols.show_absolute_time,
            "view.all" => {
                self.all_branches = !self.all_branches;
                self.reload_history();
                self.show_message(
                    if self.all_branches { "every branch" } else { "the checked-out branch" },
                    false,
                );
            }
            _ => return false,
        }
        true
    }

    /// The actions of the diff preview: marks, folds and whitespace.
    pub(super) fn preview_action(&mut self, id: &str) -> bool {
        match id {
            "preview.next_hunk" => self.jump_to_next_mark(DiffMark::Hunk),
            "preview.prev_hunk" => self.jump_to_prev_mark(DiffMark::Hunk),
            "preview.next_file" => self.jump_to_next_mark(DiffMark::File),
            "preview.prev_file" => self.jump_to_prev_mark(DiffMark::File),
            "preview.fold" => self.fold_file(),
            "preview.whitespace" => {
                let on = !pv::diff_ignoring_whitespace();
                pv::diff_ignore_whitespace(on);
                self.preview = None;
                self.show_message(
                    if on { "ignoring whitespace changes" } else { "showing whitespace changes" },
                    false,
                );
            }
            _ => return false,
        }
        true
    }
}
