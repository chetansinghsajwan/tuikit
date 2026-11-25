//! Drawing, and what drawing needs: git signs, folder counts, the footer.

use super::*;

impl Panel {
    /// The git letter and kind to show for a row, if it has a change.
    pub(super) fn git_state(&self, row: &Row) -> Option<(char, Kind)> {
        let (repo, status) = self.git.as_ref()?;
        let rel_path = repo.relative(&row.path)?;
        if row.is_dir {
            status.dir(&rel_path).map(|k| (k.letter(), k))
        } else {
            status.get(&rel_path)?.sign()
        }
    }

    /// Every state beneath a folder, most serious first (a file has one sign, see `git_state`).
    pub(super) fn git_dir_kinds(&self, row: &Row) -> Vec<Kind> {
        let Some((repo, status)) = self.git.as_ref() else { return Vec::new() };
        match repo.relative(&row.path) {
            Some(rel_path) if row.is_dir => status.dir_kinds(&rel_path),
            _ => Vec::new(),
        }
    }

    /// Start counting the visible folders' sizes (a few at a time) while the size column is on.
    pub(super) fn count_visible_dirs(&mut self) {
        if !matches!(self.opts.columns, Columns::Full | Columns::SizeTime) {
            return;
        }
        let mut running = self.dir_sizes.values().filter(|d| !d.state().1).count();
        let visible: Vec<PathBuf> = self
            .list
            .shown()
            .iter()
            .filter_map(|&i| self.rows.get(i))
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
        self.list
            .shown()
            .iter()
            .filter_map(|&i| self.rows.get(i))
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

    /// The text shown before the typed input for each kind of prompt.
    pub(super) fn prompt_label(kind: &PromptKind) -> &'static str {
        match kind {
            PromptKind::Create(_) => "new: ",
            PromptKind::Rename(_) => "rename: ",
            PromptKind::OpenWith(_) => "open with: ",
        }
    }

    /// The footer: a prompt, a confirmation question, or the last message.
    pub(super) fn footer(&self, cx: &ui::Context) -> Option<Line<'static>> {
        let theme = cx.theme;
        let c = &theme.colors;
        match &self.mode {
            Mode::Prompt(kind, input) => {
                Some(ui::prompt::line(cx, Self::prompt_label(kind), input.value()))
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
                Some(
                    Footer::Question(format!(
                        "{verb} {what}? ({} again, Esc cancels)",
                        kind.confirm_key()
                    ))
                    .line(cx),
                )
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
            Mode::Normal if self.job.is_some() => self.job_line(cx),
            Mode::Normal => self.message.as_ref().map(|n| Footer::Notice(n).line(cx)),
        }
    }

    /// Draw the panel in `area`; `focused` colors the border like any focused pane.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, cx: &ui::Context, focused: bool) {
        let theme = cx.theme;
        self.poll_job();
        let c = &theme.colors;
        let title = self
            .root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".into());
        let icon = theme.icons.dir(&title, true);
        let pane = Pane::new().title(&title).icon(icon).focused(focused);
        let mut inner = pane.draw(cx, area, buf);
        if inner.height == 0 || inner.width < 4 {
            return;
        }

        if let Some(line) = self.footer(cx) {
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

        // The folders above the top row stay pinned at the top while scrolling, so the path is
        // never lost.
        let active = self.active_scope();
        // The list is taken out so the rows can read the panel while it scrolls.
        let mut list = std::mem::take(&mut self.list);
        let sizes = &self.dir_sizes;
        let row = |s: ui::list::Slot| {
            let (i, width) = (s.index, s.width);
            let row = &self.rows[i];
            let git = self.git_state(row);
            let submodule = row.is_dir
                && self.git.as_ref().is_some_and(|(r, s)| {
                    r.relative(&row.path).is_some_and(|p| s.is_submodule(&p))
                });
            let tint = git.map(|(_, k)| pv::kind_color(theme, k));

            // Size and modified-time columns sit left of the git sign, when there is room.
            let cols = if width >= 44 { self.opts.columns.width() } else { 0 };
            let mut trailing: Vec<Span<'static>> = Vec::new();
            if cols > 0 {
                let mode = self.opts.columns;
                let muted = Style::new().fg(c.muted.into());
                if matches!(mode, Columns::Full | Columns::PermTime) {
                    trailing.extend(meta::perm_spans(cx, &meta::perm_string(row.mode), false));
                    trailing.push(Span::styled(" ", muted));
                }
                let mut text = String::new();
                if matches!(mode, Columns::Full | Columns::SizeTime) {
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
                let room = (width / 3).max(1);
                let shown = kinds.len().min(room.div_ceil(2).max(1));
                kinds.iter().take(shown).map(|k| (k.letter(), pv::kind_color(theme, *k))).collect()
            } else if let Some((ch, kind)) = git {
                vec![(ch, pv::kind_color(theme, kind))]
            } else {
                Vec::new()
            };

            ui::tree::FileRow {
                depth: usize::from(row.depth),
                name: &row.name,
                active_guide: active.and_then(|s| s.col_at(i)),
                is_dir: row.is_dir,
                is_open: self.expanded.contains(&row.path),
                is_executable: !row.is_dir && is_executable(&row.path),
                icon: submodule.then_some(theme.icons.submodule),
                tint,
                // Marked rows: yellow (yazi's selected color) and underlined (no column is spent on a marker).
                is_marked: self.marked.contains(&row.path),
                is_cut: self.clip.as_ref().is_some_and(|k| k.cut && k.paths.contains(&row.path)),
                is_hidden: row.name.starts_with('.'),
                is_ignored: row.ignored,
                trailing,
                signs,
                ..Default::default()
            }
            .list_row(cx, width)
        };
        let pinned = |first| self.pinned(first);
        list.view(self.rows.len(), row)
            .pane(area)
            .focused(focused)
            .pinned(pinned)
            .render(cx, inner, buf);
        self.list = list;
        // The rows on screen are known now; their folder sizes start counting.
        self.count_visible_dirs();
        self.sizes_shown = self.size_signature();
    }
}
