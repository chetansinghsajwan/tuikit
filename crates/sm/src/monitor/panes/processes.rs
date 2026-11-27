//! The process pane: the table of processes, under a line with how many and what filter.

use super::super::Monitor;
use crate::format;
use crate::procs::Sort;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ui::table::{Cell, Column, Width};
use ui::{Component, Pane};

impl Monitor {
    /// The process pane: a line of facts (how many, the filter), then the rows with the cursor. The
    /// sort shows in the header of its column.
    pub(in crate::monitor) fn draw_procs(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        cx: &ui::Context,
    ) {
        let theme = cx.theme;
        let rows = self.rows();
        let columns = columns();
        let pane = Pane::new().title("processes").focused(true);
        let inner = pane.draw(cx, area, buf);
        if inner.height < 3 {
            return;
        }
        let mut facts = format!("{} processes", rows.len());
        if !self.options.filter.is_empty() {
            facts.push_str(&format!("  filter {}", self.options.filter));
        }
        let muted = Style::new().fg(theme.colors.muted.into());
        let inner = Self::info(buf, inner, vec![Span::styled(facts, muted)]);
        let procs = &self.sampler.snapshot.procs;
        let row = |i: usize| {
            let p = &procs[rows[i].index];
            // The row keeps its identity (the pid) and never flashes when the numbers change.
            ui::table::Row {
                id: u64::from(p.pid) + 1,
                content: 1,
                cells: vec![
                    Cell::from(p.pid.to_string()),
                    Cell::from(p.name.clone()),
                    Cell::from(format!("{:.1}", p.cpu)).right().filled(p.cpu / 100.0),
                    Cell::from(format::bytes(p.memory)),
                    Cell::from(p.threads.map_or("-".to_string(), |n| n.to_string())),
                    Cell::from(format::rate(p.read)),
                    Cell::from(format::rate(p.written)),
                    Cell::from(format::duration(p.run_time)),
                    Cell::from(p.status.clone()),
                ],
                ..ui::table::Row::default()
            }
        };
        self.list
            .view(rows.len(), &columns, row)
            .header(true)
            .tree(|i| rows[i].depth)
            .pane(area)
            .focused(true)
            .render(cx, inner, buf);
    }
}

/// The columns of the process list. The sort moves through them in this order, each in the direction it
/// starts in (load largest first, pid and name smallest first); the right ones drop first when narrow.
pub(in crate::monitor) fn columns() -> Vec<Column> {
    let sortable = |c: Column, sort: Sort| c.sortable(sort.is_descending());
    let number = |id, title, width, priority| {
        Column::new(id, title, Width::Fixed(width)).right().muted().priority(priority)
    };
    vec![
        sortable(Column::new("pid", "PID", Width::Fixed(7)).right().muted(), Sort::Pid),
        sortable(Column::new("name", "NAME", Width::Flex(1)).min(12), Sort::Name),
        // The title and the sort mark after a blank cell take 6.
        sortable(number("cpu", "CPU%", 6, 1), Sort::Cpu),
        sortable(number("mem", "MEM", 7, 2), Sort::Memory),
        number("threads", "THREADS", 7, 5),
        number("read", "READ", 7, 6),
        number("write", "WRITE", 7, 7),
        number("time", "TIME", 7, 4),
        Column::new("state", "STATE", Width::Fixed(8)).muted().priority(3),
    ]
}
