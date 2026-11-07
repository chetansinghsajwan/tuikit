//! Drawing, and what drawing needs: git signs, folder counts, the footer.

use super::*;

impl Panel {
    pub(super) fn git_state(&self, row: &Row) -> Option<(char, Kind)> {
        let (repo, status) = self.git.as_ref()?;
        let rela = repo.relative(&row.path)?;
        if row.is_dir {
            status.dir(&rela).map(|k| (k.letter(), k))
        } else {
            status.get(&rela)?.sign()
        }
    }

    /// Every state beneath a folder, most serious first (a file has one sign, see `git_state`).
    pub(super) fn git_dir_kinds(&self, row: &Row) -> Vec<Kind> {
        let Some((repo, status)) = self.git.as_ref() else { return Vec::new() };
        match repo.relative(&row.path) {
            Some(rela) if row.is_dir => status.dir_kinds(&rela),
            _ => Vec::new(),
        }
    }

    /// Start counting the visible folders' sizes (a few at a time) while the size column is on.
    pub(super) fn count_visible_dirs(&mut self) {
        if !matches!(self.opts.linemode, Linemode::Full | Linemode::SizeTime) {
            return;
        }
        let mut running = self.dir_sizes.values().filter(|d| !d.state().1).count();
        let visible: Vec<PathBuf> = self
            .visible()
            .into_iter()
            .map(|i| &self.rows[i])
            .filter(|r| r.is_dir)
            .map(|r| r.path.clone())
            .collect();
        for p in visible {
            if running >= 3 {
                break;
            }
            if !self.dir_sizes.contains_key(&p) {
                self.dir_sizes.insert(p.clone(), meta::DirSize::spawn(p));
                running += 1;
            }
        }
    }

    /// The folder sizes of the visible rows and how far each count is.
    pub(super) fn size_signature(&self) -> Vec<(PathBuf, (u64, bool))> {
        self.visible()
            .into_iter()
            .map(|i| &self.rows[i])
            .filter_map(|r| self.dir_sizes.get(&r.path).map(|d| (r.path.clone(), d.state())))
            .collect()
    }

    /// Call between events: starts more folder counts and says whether something on
    /// screen changed by itself (a count moved), so the host knows to redraw.
    pub fn tick(&mut self) -> bool {
        self.poll_job();
        self.count_visible_dirs();
        std::mem::take(&mut self.redraw)
            || self.job.is_some()
            || self.size_signature() != self.sizes_shown
    }

    pub(super) fn prompt_label(kind: &PromptKind) -> &'static str {
        match kind {
            PromptKind::Create(_) => "new: ",
            PromptKind::Rename(_) => "rename: ",
            PromptKind::OpenWith(_) => "open with: ",
        }
    }

    /// The footer: a prompt, a confirmation question, or the last message.
    pub(super) fn footer(&self, theme: &Theme) -> Option<Line<'static>> {
        let c = &theme.colors;
        match &self.mode {
            Mode::Prompt(kind, input) => {
                Some(ui::prompt::line(theme, Self::prompt_label(kind), input.value()))
            }
            Mode::Confirm(kind, targets) => {
                let what = if targets.len() == 1 {
                    targets[0]
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                } else {
                    format!("{} items", targets.len())
                };
                let verb = if *kind == ConfirmKind::Trash { "Trash" } else { "Delete permanently" };
                Some(Line::from(Span::styled(
                    format!("{verb} {what}? ({} again, Esc cancels)", kind.confirm_key()),
                    Style::new().fg(c.warning.into()),
                )))
            }
            Mode::Bookmark(op) => {
                let verb = match op {
                    BookmarkOp::Set => "save here as",
                    BookmarkOp::Jump => "go to",
                    BookmarkOp::Delete => "remove",
                };
                let mut spans =
                    vec![Span::styled(format!("{verb}: "), Style::new().fg(c.accent.into()))];
                for (k, p) in self.bookmarks.iter() {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string());
                    spans.push(Span::styled(format!("{k}"), Style::new().fg(c.warning.into())));
                    spans.push(Span::styled(format!(" {name}  "), Style::new().fg(c.muted.into())));
                }
                Some(Line::from(spans))
            }
            Mode::Normal if self.job.is_some() => self.job_line(theme),
            Mode::Normal => self.message.as_ref().map(|(m, err)| {
                Line::from(Span::styled(
                    m.clone(),
                    Style::new().fg(if *err { c.error } else { c.muted }.into()),
                ))
            }),
        }
    }

    /// Draw the panel in `area`; `focused` colors the border like any focused pane.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme, focused: bool) {
        self.poll_job();
        let c = &theme.colors;
        let title = self
            .root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".into());
        let icon = ui::icons::dir(&title, true);
        let shown = if self.ignored_hidden > 0 {
            format!("{title}  ({} ignored hidden, I shows)", self.ignored_hidden)
        } else {
            title.clone()
        };
        let pane = Pane::new(theme).title(&shown).icon(icon).focused(focused);
        let mut inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height == 0 || inner.width < 4 {
            return;
        }

        if let Some(line) = self.footer(theme) {
            let y = inner.bottom() - 1;
            let footer_area = Rect { y, height: 1, ..inner };
            Paragraph::new(line).render(footer_area, buf);
            if let Mode::Prompt(kind, input) = &self.mode {
                ui::prompt::cursor(
                    buf,
                    footer_area,
                    Self::prompt_label(kind),
                    input.visual_cursor(),
                );
            }
            inner.height -= 1;
            if inner.height == 0 {
                return;
            }
        }

        self.page = usize::from(inner.height).max(1);
        // The folders above the top row stay pinned at the top while scrolling, so the path is
        // never lost; they take rows from the window, which the scrolling accounts for.
        if self.cursor < self.offset {
            self.offset = self.cursor;
        } else {
            while self.cursor >= self.offset + self.page - self.pinned(self.offset).len() {
                self.offset += 1;
            }
        }

        let rows = ui::list::Rows::new(area, inner);
        self.count_visible_dirs();
        let sizes = &self.dir_sizes;
        let shown = self.visible();
        let active = self.active_scope();

        for (k, &i) in shown.iter().enumerate() {
            let row = &self.rows[i];
            let y = inner.y + k as u16;
            let area = rows.content(y);
            let selected = i == self.cursor;
            let git = self.git_state(row);
            let tint = git.map(|(_, k)| pv::kind_color(theme, k));

            // Size and modified-time columns sit left of the git sign, when there is room.
            let cols = if area.width >= 44 { self.opts.linemode.width() } else { 0 };
            let mut trailing: Vec<Span<'static>> = Vec::new();
            if cols > 0 {
                let mode = self.opts.linemode;
                let muted = Style::new().fg(c.muted.into());
                if matches!(mode, Linemode::Full | Linemode::PermTime) {
                    trailing.extend(meta::perm_spans(theme, &meta::perm_string(row.mode), false));
                    trailing.push(Span::styled(" ", muted));
                }
                let mut text = String::new();
                if matches!(mode, Linemode::Full | Linemode::SizeTime) {
                    let size = if row.is_dir {
                        sizes.get(&row.path).map(|d| meta::size_cell(d.state())).unwrap_or_default()
                    } else {
                        meta::size_short(row.size)
                    };
                    text.push_str(&format!("{size:>5} "));
                }
                text.push_str(&format!("{:>8}", meta::time_relative(row.modified)));
                trailing.push(Span::styled(text, muted));
            }

            // A folder with several kinds of change shows one sign for each, the most serious on the left.
            let kinds = self.git_dir_kinds(row);
            let signs: Vec<(char, Color)> = if kinds.len() > 1 {
                let room = (usize::from(area.width) / 3).max(1);
                let shown = kinds.len().min(room.div_ceil(2).max(1));
                kinds.iter().take(shown).map(|k| (k.letter(), pv::kind_color(theme, *k))).collect()
            } else if let Some((ch, kind)) = git {
                vec![(ch, pv::kind_color(theme, kind))]
            } else {
                Vec::new()
            };

            ui::tree::file_row(
                theme,
                buf,
                &rows,
                y,
                &ui::tree::FileRow {
                    depth: usize::from(row.depth),
                    name: &row.name,
                    active_guide: active.and_then(|s| s.col_at(i)),
                    is_dir: row.is_dir,
                    open: self.expanded.contains(&row.path),
                    executable: !row.is_dir && is_executable(&row.path),
                    tint,
                    selected,
                    focused,
                    // Marked rows: yellow (yazi's selected color) and underlined (no column is spent on a marker).
                    marked: self.marked.contains(&row.path),
                    cut: self.clip.as_ref().is_some_and(|k| k.cut && k.paths.contains(&row.path)),
                    trailing,
                    signs,
                    ..Default::default()
                },
            );
        }
        self.sizes_shown = self.size_signature();
    }
}
