//! The tab strip and the screen of each tab.

use super::*;

mod list;
mod preview;
mod status;

impl App {
    /// Draw the tab strip and the screen of the active tab.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let labels = [
            ui::tabs::Label::new("Status"),
            ui::tabs::Label::new("Commits"),
            ui::tabs::Label::new("Refs"),
        ];
        let active = match self.screen {
            Screen::Status => 0,
            Screen::Commits => 1,
            Screen::Refs => 2,
        };
        ui::tabs::render(Rect { height: 1, ..area }, buf, &theme, &labels, active, true);
        let rest = Rect { y: area.y + 1, height: area.height.saturating_sub(1), ..area };
        match self.screen {
            Screen::Status => self.render_panes(rest, buf),
            Screen::Commits => self.render_commits_tab(rest, buf),
            Screen::Refs => self.render_refs_tab(rest, buf),
        }
    }

    /// The Commits tab: the whole graph of every branch full width, the diff of the commit beside it on request.
    pub(super) fn render_commits_tab(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        if self.commit_files.is_some() {
            let [left, right] = ui::layout::main_preview(&theme, body);
            self.render_commit_files(left, buf, &theme);
            self.render_preview(Focus::Commits, right, buf, &theme);
        } else if self.detail && body.width >= 60 {
            let [left, right] = ui::layout::main_preview(&theme, body);
            self.render_list(Focus::Commits, left, buf, &theme);
            self.render_preview(Focus::Commits, right, buf, &theme);
        } else {
            self.render_list(Focus::Commits, body, buf, &theme);
        }
        self.render_status(status, buf, &theme);
        self.render_overlays(area, buf, &theme);
    }

    /// The files of the commit being explored, as a tree.
    pub(super) fn render_commit_files(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let Some(cf) = self.commit_files.as_mut() else { return };
        let c = &theme.colors;
        let title = format!("{}  {}", cf.short, cf.subject);
        let pane = Pane::new(theme).title(&title).icon(ui::icons::COMMIT).focused(true);
        let inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height == 0 || inner.width < 6 {
            return;
        }
        if cf.rows.is_empty() {
            buf.set_string(
                inner.x + 1,
                inner.y,
                "no files changed",
                Style::new().fg(c.muted.into()),
            );
            return;
        }
        let page = usize::from(inner.height);
        if cf.cursor < cf.offset {
            cf.offset = cf.cursor;
        } else if cf.cursor >= cf.offset + page {
            cf.offset = cf.cursor + 1 - page;
        }
        let geo = ui::list::Rows::new(area, inner);
        let counts = ui::tree::CountWidths::of(
            cf.changes.iter().map(|f| (!f.binary).then_some((f.added, f.removed))),
        );
        let depths: Vec<usize> = cf
            .rows
            .iter()
            .map(|r| match r {
                FileRow::Folder { depth, .. } | FileRow::File { depth, .. } => *depth,
            })
            .collect();
        let scope = ui::tree::active_scope(&depths, cf.cursor);
        for (n, row) in cf.rows.iter().enumerate().skip(cf.offset).take(page) {
            let y = inner.y + (n - cf.offset) as u16;
            let selected = n == cf.cursor;
            let file_row = match row {
                FileRow::Folder { label, depth, count, folded, .. } => ui::tree::FileRow {
                    depth: *depth,
                    name: label,
                    is_dir: true,
                    open: !*folded,
                    count: Some(*count),
                    ..Default::default()
                },
                FileRow::File { change, depth } => {
                    let color = pv::kind_color(theme, change.kind);
                    // The line counts in aligned columns, left of the sign (when there is room for them).
                    let trailing = if inner.width < 30 {
                        Vec::new()
                    } else {
                        let mut t = ui::tree::line_counts(
                            theme,
                            (!change.binary).then_some((change.added, change.removed)),
                            counts,
                        );
                        t.push(Span::raw(" "));
                        t
                    };
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
            ui::tree::file_row(
                theme,
                buf,
                &geo,
                y,
                &ui::tree::FileRow {
                    selected,
                    focused: true,
                    active_guide: scope.and_then(|s| s.col_at(n)),
                    ..file_row
                },
            );
        }
    }

    /// The Status tab: the changed files and the preview of the one under the cursor.
    pub(super) fn render_panes(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        let [left, right] = ui::layout::main_preview(&theme, body);
        self.files.render_compact(left, buf, &theme, true);
        self.files.render_preview_pane(right, buf, &theme);
        self.render_status(status, buf, &theme);
        self.render_overlays(area, buf, &theme);
    }

    /// The Refs tab: branches (with remotes and tags) above the stashes, the preview beside them.
    pub(super) fn render_refs_tab(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme.clone();
        let body = Rect { height: area.height.saturating_sub(1), ..area };
        let status = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        let [left, right] = ui::layout::main_preview(&theme, body);
        // The focused pane takes the room, the other shows a few rows.
        let constraints = REFS_PANES
            .map(|f| if f == self.focus { Constraint::Min(7) } else { Constraint::Length(6) });
        let panes = Layout::vertical(constraints).split(left);
        for (pane, f) in panes.iter().zip(REFS_PANES) {
            self.render_list(f, *pane, buf, &theme);
            if f != self.focus {
                ui::dim::dim_to(buf, *pane, &theme, ui::dim::KEEP_INACTIVE);
            }
        }
        self.render_preview(self.focus, right, buf, &theme);
        self.render_status(status, buf, &theme);
        self.render_overlays(area, buf, &theme);
    }

    /// The which-key menu and the keys help, over everything else.
    pub(super) fn render_overlays(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let menu = match self.focus {
            Focus::Files => self.files.menu(),
            f => Self::list_index(f).and_then(|i| self.maps[i].menu()),
        };
        if let Some(menu) = menu {
            ui::keys::render_menu(area, buf, theme, &menu);
        }
        if let Mode::Popup(p) = &self.mode {
            p.render(area, buf, theme);
        }
        if self.help.is_some() {
            let rows = self.keys_rows();
            if let Some(h) = self.help.as_mut() {
                h.render(area, buf, theme, "Keys", &rows);
            }
        }
    }
}
