//! Drawing: the change list, the diff pane and the compact dock view.

use super::*;

impl Panel {
    /// Only the file list (no diff pane), for a narrow dock; `focused` colors
    /// the border like any focused pane.
    pub fn render_compact(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme, focused: bool) {
        self.render_list(area, buf, theme, focused);
    }

    /// Only the diff of the highlighted file, for a host that lays the panes out itself.
    pub fn render_preview_pane(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        self.render_preview(area, buf, theme);
    }

    /// Replace the list pane's title (`git: <branch>` by default).
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
    }

    /// The checked-out branch, as of the last refresh.
    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let [list_area, preview_area] = layout::main_preview(theme, area);
        self.render_list(list_area, buf, theme, true);
        self.render_preview(preview_area, buf, theme);
    }

    pub(super) fn render_list(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        focused: bool,
    ) {
        let c = &theme.colors;
        let title = self.title.clone().unwrap_or_else(|| format!("git: {}", self.branch));
        let pane = Pane::new(theme).title(&title).icon(ui::icons::GIT).focused(focused);
        let mut inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height == 0 || inner.width < 6 {
            return;
        }
        let footer = self.footer(theme);
        if !footer.is_empty() {
            let rows = (footer.len() as u16).min(inner.height);
            let top = inner.bottom() - rows;
            for (i, line) in footer
                .into_iter()
                .rev()
                .take(usize::from(rows))
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .enumerate()
            {
                Paragraph::new(line).render(Rect { y: top + i as u16, height: 1, ..inner }, buf);
            }
            if let Mode::Commit(input) = &self.mode {
                // The prompt is the last footer row.
                let (x, y) = (inner.x + 8 + input.visual_cursor() as u16, inner.bottom() - 1);
                if x < inner.right()
                    && let Some(cell) = buf.cell_mut((x, y))
                {
                    cell.set_style(Style::new().add_modifier(Modifier::REVERSED));
                }
            }
            inner.height -= rows;
            if inner.height == 0 {
                return;
            }
        }
        if self.rows.is_empty() {
            buf.set_string(
                inner.x + 1,
                inner.y,
                "clean: nothing to commit",
                Style::new().fg(c.muted.into()),
            );
            return;
        }

        self.page = usize::from(inner.height).max(1);
        let crow = self.items.get(self.cursor).copied().unwrap_or(0);
        if crow < self.offset {
            self.offset = crow;
        } else if crow >= self.offset + self.page {
            self.offset = crow + 1 - self.page;
        }
        let depths: Vec<usize> = self
            .rows
            .iter()
            .map(|r| match r {
                Row::Folder { depth, .. } | Row::File { depth, .. } => *depth,
            })
            .collect();
        let scope = ui::tree::active_scope(&depths, crow);
        let geo = ui::list::Rows::new(area, inner);
        for (i, row) in self.rows.iter().enumerate().skip(self.offset).take(self.page) {
            let y = inner.y + (i - self.offset) as u16;
            let selected = i == crow;
            match row {
                Row::Folder { label, depth, count, folded, .. } => {
                    ui::tree::file_row(
                        theme,
                        buf,
                        &geo,
                        y,
                        &ui::tree::FileRow {
                            depth: *depth,
                            name: label,
                            is_dir: true,
                            open: !*folded,
                            count: Some(*count),
                            selected,
                            focused: true,
                            active_guide: scope.and_then(|s| s.col_at(i)),
                            ..Default::default()
                        },
                    );
                }
                Row::File { change, depth } => {
                    let name = if self.tree {
                        change
                            .rela
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    } else {
                        change.rela.display().to_string()
                    };
                    // Two marks like `git status -s`: what is staged (always in the staged color, green, so a staged file
                    // is told apart at a glance), then what is not (in the color of its change).
                    let staged = |kind: Option<Kind>| {
                        kind.map_or((' ', c.fg.into()), |k| (kind_char(k), c.success.into()))
                    };
                    let unstaged = |kind: Option<Kind>| {
                        kind.map_or((' ', c.fg.into()), |k| {
                            (kind_char(k), pv::kind_color(theme, k))
                        })
                    };
                    ui::tree::file_row(
                        theme,
                        buf,
                        &geo,
                        y,
                        &ui::tree::FileRow {
                            depth: *depth,
                            name: &name,
                            tint: Some(pv::kind_color(theme, change.kind())),
                            icon: change.submodule.then_some(ui::icons::SUBMODULE),
                            selected,
                            focused: true,
                            signs: vec![staged(change.staged), unstaged(change.unstaged)],
                            active_guide: scope.and_then(|s| s.col_at(i)),
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }

    /// The preview of a submodule: where it is, then the commits that moved it.
    fn submodule_lines(&self, rela: &Path, theme: &Theme) -> Vec<Line<'static>> {
        let path = rela.to_string_lossy();
        let mut text = String::new();
        if let Some(s) = self.repo.submodules().unwrap_or_default().iter().find(|s| s.path == path)
        {
            text.push_str(&format!(
                "url     {}\ncommit  {}\nstate   {:?}{}\n\n",
                s.url,
                s.oid,
                s.state,
                if s.dirty { ", uncommitted changes inside" } else { "" }
            ));
        }
        match self.repo.submodule_summary(&path) {
            Ok(s) => text.push_str(&s),
            Err(e) => text.push_str(&e.to_string()),
        }
        let style = Style::new().fg(theme.colors.muted.into());
        text.lines().map(|l| Line::from(Span::styled(l.to_string(), style))).collect()
    }

    pub(super) fn render_preview(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let current = self.current().map(|c| (c.unstaged.is_none(), c.rela.clone(), c.submodule));
        let title = current.as_ref().map(|(_, p, _)| p.display().to_string()).unwrap_or_default();
        let name = title.trim_end_matches('/').rsplit('/').next().unwrap_or(&title).to_string();
        let pane = Pane::new(theme).title(&title).icon(ui::icons::file(&name, false));
        let inner = pane.inner(area);
        pane.render(area, buf);
        let Some((staged, rela, submodule)) = current else {
            self.preview = None;
            return;
        };
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let key = (rela.clone(), staged, inner.width, inner.height, self.version);
        if self.preview.as_ref().map(|p| &p.key) != Some(&key) {
            let opts =
                pv::Options { width: Some(inner.width), height: None, git: false, focus: None };
            let side = if staged { Side::Staged } else { Side::Unstaged };
            let (lines, hunks) = if submodule {
                (self.submodule_lines(&rela, theme), Vec::new())
            } else {
                match pv::preview_diff_marked(&self.root, &rela, side, theme, &opts) {
                    Ok((l, a)) => (l, a.hunks),
                    Err(e) => (
                        vec![Line::from(Span::styled(
                            e.to_string(),
                            Style::new().fg(theme.colors.muted.into()),
                        ))],
                        Vec::new(),
                    ),
                }
            };
            self.preview = Some(Preview { key, lines, hunks });
            self.pscroll = 0;
        }
        self.pheight = usize::from(inner.height);
        if let Some(p) = &self.preview {
            self.pscroll = self.pscroll.min(p.lines.len().saturating_sub(1));
            let shown: Vec<Line<'static>> = p
                .lines
                .iter()
                .skip(self.pscroll)
                .take(usize::from(inner.height))
                .cloned()
                .collect();
            Paragraph::new(shown).render(inner, buf);
        }
    }
}
