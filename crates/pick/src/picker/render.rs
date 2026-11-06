//! Drawing: the prompt, the list (flat or as a hierarchy), the preview.

use super::*;

impl Picker {
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let (list_area, preview_area) = if self.opts.preview {
            let [l, p] = layout::main_preview(theme, area);
            (l, Some(p))
        } else {
            (area, None)
        };
        self.render_list(list_area, buf, theme);
        if let Some(p) = preview_area {
            self.render_preview(p, buf, theme);
        }
    }

    /// Spans for one row: fuzzy rows highlight matched characters; grep rows
    /// show `path:line:` then the line with the match emphasized.
    ///
    /// With `leaf` (the hierarchy view) the folders above the item are already
    /// drawn as header rows, so only the file name (or `line:` for a match) shows.
    pub(super) fn row_spans(&mut self, cand: &Candidate, name_fg: Color, theme: &Theme, snap_index: u32, leaf: bool) -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        // Row numbers and, for input that carries times, how long ago each was.
        if self.row_style.numbers {
            let width = self.nucleo.snapshot().matched_item_count().max(1).to_string().len();
            spans.push(Span::styled(format!("{:>width$} ", snap_index + 1), Style::new().fg(theme.colors.muted.into())));
        }
        if self.timestamps {
            let age = cand.time.map(ui::time::ago).unwrap_or_default();
            spans.push(Span::styled(format!("{age:>8}  "), Style::new().fg(theme.colors.muted.into())));
        }
        // Path rows get a file icon; a grep match under its file header (the hierarchy view) does not.
        if self.opts.paths && !(leaf && cand.loc.is_some()) {
            let path = cand.path();
            let icon = match path.strip_suffix('/') {
                Some(dir) => ui::icons::dir(dir.rsplit('/').next().unwrap_or(dir), false),
                None => ui::icons::file(path.rsplit('/').next().unwrap_or(path), false),
            };
            spans.push(Span::styled(format!("{} ", icon.glyph), Style::new().fg(icon.color)));
        }
        spans.extend(self.row_spans_inner(cand, name_fg, theme, snap_index, leaf));
        spans
    }

    pub(super) fn row_spans_inner(&mut self, cand: &Candidate, name_fg: Color, theme: &Theme, snap_index: u32, leaf: bool) -> Vec<Span<'static>> {
        let c = &theme.colors;
        let hit = Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD);
        if let Some(loc) = &cand.loc {
            let mut spans = Vec::new();
            if !leaf {
                spans.push(Span::styled(loc.path.clone(), Style::new().fg(name_fg)));
                spans.push(Span::styled(format!(":{}: ", loc.line), Style::new().fg(c.muted.into())));
            } else {
                spans.push(Span::styled(format!("{}: ", loc.line), Style::new().fg(c.muted.into())));
            }
            let (a, b) = (loc.hit.start, loc.hit.end);
            if a < b && b <= loc.text.len() {
                spans.push(Span::styled(loc.text[..a].to_string(), Style::new().fg(c.fg.into())));
                spans.push(Span::styled(loc.text[a..b].to_string(), hit));
                spans.push(Span::styled(loc.text[b..].to_string(), Style::new().fg(c.fg.into())));
            } else {
                spans.push(Span::styled(loc.text.clone(), Style::new().fg(c.fg.into())));
            }
            return spans;
        }
        let snap = self.nucleo.snapshot();
        let mut indices = Vec::new();
        if let Some(item) = snap.get_matched_item(snap_index) {
            snap.pattern().column_pattern(0).indices(item.matcher_columns[0].slice(..), &mut self.matcher, &mut indices);
        }
        indices.sort_unstable();
        indices.dedup();
        let mut hits = indices.iter().copied().peekable();
        // Git items: the name part takes its kind color, the subject the text color.
        let (name_len, name_color, rest_color) = match &cand.git {
            Some(g) => (g.name.chars().count(), Self::git_item_color(theme, g), Color::from(c.fg)),
            None => (usize::MAX, name_fg, name_fg),
        };
        let mut spans = Vec::new();
        if let Some(g) = &cand.git {
            spans.push(Span::styled(if g.is_head { "* " } else { "  " }, Style::new().fg(c.accent.into())));
        }
        if let Some(b) = &cand.buffer {
            spans.push(Span::styled(format!("{:<2} ", b.flags), Style::new().fg(c.warning.into())));
        }
        // In the hierarchy only the file name is drawn; match positions still refer to the full path.
        let skip = if leaf { cand.display.rfind('/').map(|i| cand.display[..=i].chars().count()).unwrap_or(0) } else { 0 };
        // Code colors for the whole text, when the rows are code.
        let colored: Vec<Style> = match (&self.row_style.syntax, cand.git.is_none() && cand.loc.is_none()) {
            (Some(lang), true) => pv::highlight_line(&cand.display, lang, theme).into_iter().flat_map(|(st, piece)| piece.chars().map(move |_| st).collect::<Vec<_>>()).collect(),
            _ => Vec::new(),
        };
        for (i, ch) in cand.display.chars().enumerate() {
            let is_hit = hits.peek() == Some(&(i as u32));
            if is_hit {
                hits.next();
            }
            if i < skip {
                continue;
            }
            let base = if i < name_len { name_color } else { rest_color };
            let plain = colored.get(i).copied().unwrap_or_else(|| Style::new().fg(base));
            spans.push(Span::styled(ch.to_string(), if is_hit { hit } else { plain }));
        }
        spans
    }

    /// The lines of the grep filters: the toggles (lit when on) with the keys of the file filters, and the include
    /// and exclude lines while they hold something or take the typing.
    pub(super) fn grep_filter_rows(&self) -> Vec<Field> {
        let mut rows = vec![Field::Query];
        for (field, input) in [(Field::Include, &self.include), (Field::Exclude, &self.exclude)] {
            if self.field == field || !input.value().is_empty() {
                rows.push(field);
            }
        }
        rows
    }

    pub(super) fn render_grep_filters(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let on = |b: bool| if b { Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD | Modifier::UNDERLINED) } else { Style::new().fg(c.muted.into()) };
        let muted = Style::new().fg(c.muted.into());
        let o = &self.grep_opts;
        for (i, field) in self.grep_filter_rows().into_iter().enumerate() {
            let row = Rect { y: area.y + i as u16, height: 1, ..area };
            if field == Field::Query {
                let mut spans = vec![Span::styled("Aa", on(o.case_sensitive)), Span::raw(" "), Span::styled("ab", on(o.whole_word)), Span::raw(" "), Span::styled(".*", on(o.regex))];
                spans.push(Span::styled("   alt-c w r  ·  alt-i include  alt-e exclude", muted.add_modifier(Modifier::DIM)));
                Paragraph::new(Line::from(spans)).render(row, buf);
                continue;
            }
            let (label, input) = if field == Field::Include { ("include ", &self.include) } else { ("exclude ", &self.exclude) };
            let focused = self.field == field;
            let line = Line::from(vec![Span::styled(label, if focused { Style::new().fg(c.accent.into()) } else { muted }), Span::styled(input.value().to_string(), Style::new().fg(c.fg.into()))]);
            Paragraph::new(line).render(row, buf);
            if focused {
                ui::prompt::cursor(buf, row, label, input.visual_cursor());
            }
        }
    }


    pub(super) fn render_list(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let (snap_total, snap_matched) = {
            let s = self.nucleo.snapshot();
            (s.item_count(), s.matched_item_count())
        };
        // In grep mode every item matches, so only the item count is meaningful.
        let count = if self.opts.mode == Mode::Grep || self.live.is_some() { format!("{snap_total}") } else { format!("{snap_matched}/{snap_total}") };
        let title = self.title.as_deref().unwrap_or(if self.opts.mode == Mode::Grep { "grep" } else { "pick" });
        let mut pane = Pane::new(theme).title(title).focused(true);
        // An explicit icon, else one that fits the title (`Files`, `History`, `Branches`, ...).
        if let Some(icon) = self.icon.or_else(|| ui::icons::for_title(title)) {
            pane = pane.icon(icon);
        }
        let inner = pane.inner(area);
        pane.render(area, buf);
        if inner.height < 2 {
            return;
        }

        // Prompt row: "> query" with the count flush right.
        let cw = count.len() as u16;
        let show_count = inner.width > cw + 4;
        // The query stops one cell short of the count.
        let prompt_area = Rect { height: 1, width: if show_count { inner.width - cw - 1 } else { inner.width }, ..inner };
        let prompt = ui::prompt::line(theme, "> ", self.input.value());
        Paragraph::new(prompt).render(prompt_area, buf);
        if show_count {
            buf.set_string(inner.right() - cw, inner.y, &count, Style::new().fg(c.muted.into()));
        }
        ui::prompt::cursor(buf, prompt_area, "> ", self.input.visual_cursor());

        // A rule under the prompt, then the header lines, then the list.
        let sep = u16::from(self.separator && inner.height > 3);
        if sep == 1 {
            // The rule joins the pane's side borders (`├───┤`), in the focused border color.
            let y = inner.y + 1;
            let rule = format!("├{}┤", "─".repeat(usize::from(area.width).saturating_sub(2)));
            buf.set_string(area.x, y, rule, Style::new().fg(c.border_focused.into()));
        }
        // In grep mode the toggles and the file filters take the row under the rule.
        let filter_row = if self.opts.mode == Mode::Grep && inner.height >= 6 { self.grep_filter_rows().len() as u16 } else { 0 };
        if filter_row > 0 {
            self.render_grep_filters(Rect { y: inner.y + 1 + sep, height: filter_row, ..inner }, buf, theme);
        }
        let header_rows = (self.header.len() as u16).min(inner.height.saturating_sub(2 + sep + filter_row));
        for (i, h) in self.header.iter().take(usize::from(header_rows)).enumerate() {
            buf.set_stringn(inner.x + 2, inner.y + 1 + sep + filter_row + i as u16, h, usize::from(inner.width.saturating_sub(2)), Style::new().fg(c.muted.into()));
        }
        let list = Rect { y: inner.y + 1 + sep + filter_row + header_rows, height: inner.height - 1 - sep - filter_row - header_rows, ..inner };
        let rows = ui::list::Rows::new(area, inner);
        self.page = u32::from(list.height).max(1);
        self.refresh_tree();
        if self.tree.is_some() {
            self.render_tree_rows(list, rows, buf, theme);
            return;
        }
        // Keep the cursor inside the window.
        if self.cursor < self.offset {
            self.offset = self.cursor;
        } else if self.cursor >= self.offset + self.page {
            self.offset = self.cursor + 1 - self.page;
        }

        let end = (self.offset + self.page).min(snap_matched);
        for (row, idx) in (self.offset..end).enumerate() {
            let Some(cand) = self.nucleo.snapshot().get_matched_item(idx).map(|i| i.data.clone()) else { break };
            let y = list.y + row as u16;
            let selected = idx == self.cursor;
            let git = self.git_state(cand.path());
            let marked = self.marked.contains(&cand.output());
            let mut spans = vec![Span::styled(
                if marked { "● " } else { "  " },
                Style::new().fg(c.accent.into()),
            )];
            let name_fg = git.map(|(_, k)| Self::git_color(theme, k)).unwrap_or(c.fg.into());
            spans.extend(self.row_spans(&cand, name_fg, theme, idx, false));
            ui::list::ListRow {
                id: ui::anim::hash_of(&cand.output()),
                left: spans,
                signs: git.map(|(ch, k)| (ch, Self::git_color(theme, k))).into_iter().collect(),
                sign_cell: true,
                tint: git.map(|(_, k)| Self::git_color(theme, k)),
                selected,
                focused: true,
                ..Default::default()
            }
            .draw(theme, buf, &rows, y);
        }
    }

    /// The list as a directory hierarchy: folder header rows with the items
    /// indented beneath them. `offset` counts rows here, not results.
    pub(super) fn render_tree_rows(&mut self, list: Rect, rows_geo: ui::list::Rows, buf: &mut Buffer, theme: &Theme) {
        let c = &theme.colors;
        let (rows, scope): (Vec<(usize, crate::tree::Row)>, Option<ui::tree::Scope>) = {
            let Some(tree) = self.tree.as_ref() else { return };
            // The folders above the top row stay pinned at the top, so scrolling never loses the
            // path; they take lines from the window, which the scrolling below accounts for.
            let room = |off: u32| (self.page as usize).saturating_sub(crate::tree::ancestors(&tree.rows, off as usize).len()).max(1) as u32;
            let crow = tree.row_of.get(&self.cursor).copied().unwrap_or(0) as u32;
            if crow < self.offset {
                self.offset = crow;
            } else {
                while crow >= self.offset + room(self.offset) {
                    self.offset += 1;
                }
            }
            let mut pinned: Vec<(usize, crate::tree::Row)> = crate::tree::ancestors(&tree.rows, self.offset as usize).into_iter().map(|i| (i, tree.rows[i].clone())).collect();
            // In a tiny pane keep at least one scrolling row: drop the outermost pins first.
            let keep = (self.page as usize).saturating_sub(1);
            if pinned.len() > keep {
                pinned.drain(..pinned.len() - keep);
            }
            let take = (self.page as usize).saturating_sub(pinned.len());
            // The guide of the scope the cursor is in is highlighted, as in the files panel.
            let depths: Vec<usize> = tree.rows.iter().map(|r| match r { crate::tree::Row::Dir { depth, .. } | crate::tree::Row::Item { depth, .. } => usize::from(*depth) }).collect();
            let scope = ui::tree::active_scope(&depths, crow as usize);
            let window = tree.rows.iter().enumerate().skip(self.offset as usize).take(take).map(|(i, r)| (i, r.clone()));
            (pinned.into_iter().chain(window).collect(), scope)
        };

        for (i, (at, row)) in rows.iter().enumerate() {
            let guide = scope.and_then(|s| s.col_at(*at));
            let y = list.y + i as u16;
            match row {
                crate::tree::Row::Dir { depth, label } => {
                    // In grep mode the header is a file, otherwise a folder.
                    let last = label.rsplit('/').next().unwrap_or(label);
                    let icon = if self.opts.mode == Mode::Grep { ui::icons::file(last, false) } else { ui::icons::dir(last, true) };
                    let mut spans = vec![Span::raw("  ")];
                    spans.extend(ui::tree::guides_marked(theme, usize::from(*depth), guide));
                    spans.push(Span::styled(format!("{} ", icon.glyph), Style::new().fg(icon.color)));
                    spans.push(Span::styled(label.clone(), Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD)));
                    ui::list::ListRow { id: ui::anim::hash_of(&(depth, label)), left: spans, focused: true, ..Default::default() }.draw(theme, buf, &rows_geo, y);
                }
                crate::tree::Row::Item { idx, depth } => {
                    let Some(cand) = self.nucleo.snapshot().get_matched_item(*idx).map(|it| it.data.clone()) else { continue };
                    let selected = *idx == self.cursor;
                    let git = self.git_state(cand.path());
                    let marked = self.marked.contains(&cand.output());
                    let mut spans = vec![Span::styled(if marked { "● " } else { "  " }, Style::new().fg(c.accent.into()))];
                    spans.extend(ui::tree::guides_marked(theme, usize::from(*depth), guide));
                    let name_fg = git.map(|(_, k)| Self::git_color(theme, k)).unwrap_or(c.fg.into());
                    spans.extend(self.row_spans(&cand, name_fg, theme, *idx, true));
                    ui::list::ListRow {
                        id: ui::anim::hash_of(&cand.output()),
                        left: spans,
                        signs: git.map(|(ch, k)| (ch, Self::git_color(theme, k))).into_iter().collect(),
                        sign_cell: true,
                        tint: git.map(|(_, k)| Self::git_color(theme, k)),
                        selected,
                        focused: true,
                        ..Default::default()
                    }
                    .draw(theme, buf, &rows_geo, y);
                }
            }
        }
    }

    pub(super) fn render_preview(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let cand = self.current();
        let title = cand
            .as_ref()
            .map(|c| match (&c.loc, &c.git) {
                (Some(l), _) => format!("{}:{}", l.path, l.line),
                (_, Some(g)) => g.name.clone(),
                _ => c.display.clone(),
            })
            .unwrap_or_default();
        // The file picker's preview has no title: the list already names the file. A commit keeps its title.
        let mut pane = Pane::new(theme);
        if let Some(c) = cand.as_ref() {
            if c.git.is_some() {
                pane = pane.title(&title).icon(ui::icons::GIT);
            } else if !self.opts.paths {
                pane = pane.title(&title);
            }
        }
        let inner = pane.inner(area);
        pane.render(area, buf);
        let Some(cand) = cand else {
            self.preview = None;
            return;
        };
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let line = cand.loc.as_ref().map(|l| l.line).unwrap_or(0);
        let id = cand.git.as_ref().map(|g| g.oid.clone()).unwrap_or_else(|| cand.path().to_string());
        let key = (id, line, inner.width, inner.height);
        if self.preview.as_ref().map(|p| &p.key) != Some(&key) {
            let opts = pv::Options {
                width: Some(inner.width),
                height: if cand.git.is_some() { None } else { Some(inner.height) },
                git: self.opts.git,
                focus: cand.loc.as_ref().map(|l| l.line).or(cand.buffer.as_ref().and_then(|b| b.focus)),
            };
            let rendered = match (&cand.git, &cand.buffer) {
                (Some(g), _) => pv::preview_commit(&self.opts.root, &g.oid, theme, &opts),
                // An open buffer: its text as shown in the editor, unsaved changes included.
                (None, Some(b)) => pv::preview_text(std::path::Path::new(cand.path()), b.text.as_bytes(), theme, &opts),
                // Plain text with an external preview command: its output, colors kept.
                (None, None) if self.preview_cmd.is_some() && cand.loc.is_none() => {
                    let line = cand.raw.as_deref().unwrap_or(&cand.display);
                    let out = self.preview_cmd.as_ref().map(|p| p.run(line)).unwrap_or_default();
                    Ok(ui::ansi::parse(&out))
                }
                (None, None) => pv::preview(&self.opts.root.join(cand.path()), theme, &opts),
            };
            let lines = match rendered {
                Ok(l) => l,
                Err(e) => vec![Line::from(Span::styled(e.to_string(), Style::new().fg(theme.colors.muted.into())))],
            };
            self.preview = Some(Preview { key, lines });
            self.pscroll = 0;
        }
        self.pheight = usize::from(inner.height);
        if let Some(p) = &self.preview {
            // Commit and command previews scroll; the rest are already windowed to the pane.
            let scrolls = cand.git.is_some() || (self.preview_cmd.is_some() && cand.loc.is_none() && cand.buffer.is_none());
            let skip = if scrolls { self.pscroll.min(p.lines.len().saturating_sub(1)) } else { 0 };
            if scrolls {
                self.pscroll = skip;
            }
            let shown: Vec<Line<'static>> = p.lines.iter().skip(skip).take(usize::from(inner.height)).cloned().collect();
            Paragraph::new(shown).render(inner, buf);
        }
    }
}
