//! What the list shows: hidden and ignored files, the columns, tables as text, refresh.

use super::*;

impl Panel {
    /// Runs one of this domain's actions by id.
    pub(super) fn run_view_action(&mut self, id: &str) -> Outcome {
        match id {
            "view.ignored" => {
                self.show_ignored = !self.show_ignored;
                self.rebuild();
            }
            "view.hidden" => {
                self.opts.hidden = !self.opts.hidden;
                self.rebuild();
            }
            "view.tables_as_text" => {
                pv::set_raw_tables(!pv::raw_tables());
                self.show_message(
                    if pv::raw_tables() { "tables show as text" } else { "tables show as tables" },
                    false,
                );
            }
            "view.columns" => {
                self.opts.columns = self.opts.columns.cycle();
                self.show_message(self.opts.columns.label(), false);
            }
            "view.refresh" => self.refresh(),
            _ => {}
        }
        Outcome::None
    }
}
