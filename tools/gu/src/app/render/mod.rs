//! The tab strip and the screen of each tab.

use super::*;

/// Drawing the list panes.
mod list;
/// Drawing the preview pane.
mod preview;
/// Drawing the status line.
mod status;

impl App {
    /// Draw the tab strip and the screen of the active tab.
    pub fn render(&mut self, cx: &ui::Context, area: Rect, buf: &mut Buffer) {
        let labels = [
            ui::tabs::Label::new("Status"),
            ui::tabs::Label::new("Commits"),
            ui::tabs::Label::new("Refs"),
        ];
        let active = match self.tab {
            Tab::Status => 0,
            Tab::Commits => 1,
            Tab::Refs => 2,
        };
        ui::tabs::render(Rect { height: 1, ..area }, buf, cx, &labels, active, true);
        let rest = Rect { y: area.y + 1, height: area.height.saturating_sub(1), ..area };
        match self.tab {
            Tab::Status => self.render_panes(rest, buf, cx),
            Tab::Commits => self.render_commits_tab(rest, buf, cx),
            Tab::Refs => self.render_refs_tab(rest, buf, cx),
        }
    }

    /// The Commits tab: the whole graph of every branch full width, the diff of the commit beside it on request.
    pub(super) fn render_commits_tab(&mut self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        if self.commit_files.is_some() {
            let [left, right] = ui::layout::main_preview(cx, body);
            self.render_commit_files(left, buf, cx);
            self.render_preview(Focus::Commits, right, buf, cx);
        } else if self.detail && body.width >= 60 {
            let [left, right] = ui::layout::main_preview(cx, body);
            self.render_list(Focus::Commits, left, buf, cx);
            self.render_preview(Focus::Commits, right, buf, cx);
        } else {
            self.render_list(Focus::Commits, body, buf, cx);
        }
        self.render_status(status, buf, cx);
        self.render_overlays(area, buf, cx);
    }

    /// The files of the commit being explored, as a tree.
    pub(super) fn render_commit_files(&mut self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let theme = cx.theme;
        let Some(cf) = self.commit_files.as_mut() else { return };
        let title = format!("{}  {}", cf.short, cf.subject);
        let pane = Pane::new().title(&title).icon(theme.icons.commit).focused(true);
        let inner = pane.draw(cx, area, buf);
        if inner.height == 0 || inner.width < 6 {
            return;
        }
        let counts = ui::tree::CountWidths::of(
            cf.changes.iter().map(|f| (!f.binary).then_some((f.added, f.removed))),
        );
        let rows = &cf.rows;
        let row = |i: usize| {
            let file_row = match &rows[i] {
                FileRow::Folder { label, depth, count, folded, .. } => ui::tree::FileRow {
                    depth: *depth,
                    name: label,
                    is_dir: true,
                    is_open: !*folded,
                    count: Some(*count),
                    ..Default::default()
                },
                FileRow::File { change, depth } => {
                    let color = pv::kind_color(theme, change.kind);
                    // The line counts in aligned columns, left of the sign (the table drops them when
                    // there is no room).
                    let trailing = ui::tree::line_counts(
                        cx,
                        (!change.binary).then_some((change.added, change.removed)),
                        counts,
                    );
                    ui::tree::FileRow {
                        depth: *depth,
                        name: change.path.rsplit('/').next().unwrap_or(&change.path),
                        tint: Some(color),
                        trailing,
                        signs: vec![(change.kind.letter(), color)],
                        ..Default::default()
                    }
                }
            };
            file_row.row(cx)
        };
        let columns = ui::tree::file_columns();
        cf.list
            .view(rows.len(), &columns, row)
            .tree(|i| match &rows[i] {
                FileRow::Folder { depth, .. } | FileRow::File { depth, .. } => *depth,
            })
            .pane(area)
            .focused(true)
            .empty("no files changed")
            .render(cx, inner, buf);
    }

    /// The Status tab: the changed files and the preview of the one under the cursor.
    pub(super) fn render_panes(&mut self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        let [left, right] = ui::layout::main_preview(cx, body);
        self.files.render_compact(left, buf, cx, true);
        self.files.render_preview_pane(right, buf, cx);
        self.render_status(status, buf, cx);
        self.render_overlays(area, buf, cx);
    }

    /// The Refs tab: branches (with remotes and tags) above the stashes, the preview beside them.
    pub(super) fn render_refs_tab(&mut self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        let [left, right] = ui::layout::main_preview(cx, body);
        let focused = REFS_PANES.iter().position(|&f| f == self.focus).unwrap_or(0);
        let panes = ui::layout::focus_stack(cx, left, REFS_PANES.len(), focused);
        for (pane, f) in panes.iter().zip(REFS_PANES) {
            self.render_list(f, *pane, buf, cx);
        }
        ui::dim::unfocused(buf, cx, &panes, focused);
        self.render_preview(self.focus, right, buf, cx);
        self.render_status(status, buf, cx);
        self.render_overlays(area, buf, cx);
    }

    /// The popup over everything else (the shell draws the menu and the help above it).
    pub(super) fn render_overlays(&mut self, area: Rect, buf: &mut Buffer, cx: &ui::Context) {
        if let Mode::Popup(p) = &self.mode {
            p.render(area, buf, cx);
        }
    }
}
