//! The lists: refs, commits, stashes, files.

use super::super::*;

impl App {
    /// Draw one list pane (refs, commits, stashes or submodules) with its title, rows and cursor.
    pub(in crate::app) fn render_list(
        &mut self,
        which: Focus,
        area: Rect,
        buf: &mut Buffer,
        cx: &ui::Context,
    ) {
        let theme = cx.theme;
        let (title, n) = match which {
            Focus::Branches => ("Refs", self.ref_rows.len()),
            Focus::Commits => ("Commits", self.commits.len()),
            Focus::Submodules => ("Submodules", self.submodules.len()),
            _ => ("Stashes", self.stashes.len()),
        };
        let focused = self.focus == which;
        let mut pane = Pane::new().title(title).focused(focused);
        let icon = if which == Focus::Branches {
            Some(theme.icons.branch)
        } else {
            theme.icons.title(title)
        };
        if let Some(icon) = icon {
            pane = pane.icon(icon);
        }
        let mut inner = pane.draw(cx, area, buf);
        if inner.height == 0 || inner.width < 4 {
            return;
        }
        // The filters in force, on a row of their own above the list.
        if which == Focus::Commits && self.filter.active() {
            let c = &theme.colors;
            let mut spans = vec![Span::styled(
                "filter ",
                Style::new().fg(c.accent.into()).add_modifier(Modifier::BOLD),
            )];
            for (i, (what, value)) in self.filter.parts().into_iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled("  ·  ", Style::new().fg(c.muted.into())));
                }
                spans.push(Span::styled(format!("{what} "), Style::new().fg(c.muted.into())));
                spans.push(Span::styled(value, Style::new().fg(c.warning.into())));
            }
            Paragraph::new(Line::from(spans)).render(Rect { height: 1, ..inner }, buf);
            inner.y += 1;
            inner.height -= 1;
            if inner.height == 0 {
                return;
            }
        }
        let Some(li) = Self::list_index(which) else { return };
        if which == Focus::Commits {
            self.fit_columns(inner.width, cx);
        }
        // The list is taken out so the rows can borrow `self` while it scrolls.
        let mut list = std::mem::take(&mut self.lists[li]);
        let row = |i: usize| {
            let (left, right) = self.row(which, i, cx);
            ui::table::Row::new(vec![left.spans.into(), right.into()])
        };
        let columns = [ui::table::Column::list().min(12), ui::table::Column::trailing()];
        list.view(n, &columns, row)
            .pane(area)
            .focused(focused)
            .empty("nothing here")
            .render(cx, inner, buf);
        self.lists[li] = list;
    }

    /// Give the flexible columns their widths for a list `width` wide: the refs column as wide as the most
    /// refs on one commit need (as far as the message keeps room), and when the message is hidden the
    /// refs (or the author names) take what is left, so the row is always filled.
    pub(in crate::app) fn fit_columns(&mut self, width: u16, cx: &ui::Context) {
        /// The least width the commit message keeps before the refs column has to shrink.
        const MIN_SUBJECT: usize = 24;
        // Every column has one width, whatever the commits are: the refs column too (when any commit
        // has a ref).
        let refs_need = *self.refs_need.get_or_insert_with(|| {
            self.commits
                .iter()
                .filter_map(|k| self.labels.get(&k.oid))
                .map(|l| {
                    pills(l)
                        .iter()
                        .map(|(r, remotes)| ref_width(&cx.theme.icons, r, remotes))
                        .sum::<usize>()
                })
                .max()
                .unwrap_or(0)
        });
        let width = usize::from(width);
        let mut fixed = if self.graph { self.graph_w + 1 } else { 0 };
        if self.cols.show_hash {
            fixed += HASH_W + 1;
        }
        if self.cols.show_relative_time {
            fixed += 9;
        }
        if self.cols.show_absolute_time {
            fixed += 18;
        }
        let name_w = if self.cols.show_author_name { AUTHOR_COL } else { 2 };
        if self.cols.show_author {
            fixed += name_w + 1;
        }
        self.author_w = name_w;
        let free = width.saturating_sub(fixed);
        let refs_on = self.cols.show_refs && refs_need > 0;
        self.refs_w = match (refs_on, self.cols.show_subject) {
            (false, _) => 0,
            (true, true) => refs_need.min(free.saturating_sub(MIN_SUBJECT)).max(8.min(refs_need)),
            // No message: the refs take the rest of the row.
            (true, false) => free.saturating_sub(1).max(refs_need.min(free)),
        };
        if !self.cols.show_subject
            && !refs_on
            && self.cols.show_author
            && self.cols.show_author_name
        {
            // Only names are left to fill the row.
            self.author_w = free.max(AUTHOR_COL);
        }
    }

    /// The text of row `i`: the entry, and its age for the right edge.
    pub(in crate::app) fn row(
        &self,
        which: Focus,
        i: usize,
        cx: &ui::Context,
    ) -> (Line<'static>, String) {
        let theme = cx.theme;
        let c = &theme.colors;
        let fg = Style::new().fg(c.fg.into());
        let muted = Style::new().fg(c.muted.into());
        match which {
            Focus::Branches => {
                let accent = Style::new().fg(c.accent.into());
                match &self.ref_rows[i] {
                    RefRow::Section { key, label, count } => {
                        let mut spans = ui::tree::group(
                            cx,
                            0,
                            self.folded.contains(key),
                            None,
                            label,
                            accent.add_modifier(Modifier::BOLD),
                        );
                        spans.push(ui::tree::count(cx, *count));
                        (Line::from(spans), String::new())
                    }
                    RefRow::Folder { key, name, depth, count } => {
                        let mut spans = ui::tree::dir(cx, *depth, name, !self.folded.contains(key));
                        spans.push(ui::tree::count(cx, *count));
                        (Line::from(spans), String::new())
                    }
                    RefRow::Item { index, name, depth } => {
                        let b = &self.branches[*index];
                        let style = if b.is_head {
                            Style::new().fg(c.success.into()).add_modifier(Modifier::BOLD)
                        } else if b.kind == RefKind::Tag {
                            Style::new().fg(c.warning.into())
                        } else {
                            fg
                        };
                        // Indentation guides, like the files tree: a thin line at each level.
                        (
                            Line::from(vec![
                                ui::tree::guides(cx, *depth),
                                Span::styled(name.clone(), style),
                            ]),
                            ago(b.time),
                        )
                    }
                }
            }
            Focus::Commits => {
                let k = &self.commits[i];
                // `hash AB subject`: the author as two initials in a color of their own, like lazygit.
                let who = if self.cols.show_author_name {
                    let mut n: String = k.author.chars().take(self.author_w).collect();
                    n.push_str(&" ".repeat(self.author_w.saturating_sub(n.chars().count())));
                    n
                } else {
                    format!("{:<2}", initials(&k.author))
                };
                let author = Span::styled(
                    format!("{who} "),
                    Style::new().fg(author_color(theme, &k.author)),
                );
                let hit = self.commit_matches(i);
                let mut spans = Vec::new();
                if let Some(row) = self.graph_rows.get(i) {
                    spans.extend(self.graph_spans(row, cx));
                }
                // A table: graph, author, hash, refs (pills; the checked-out branch is the solid one
                // with the branch icon), subject; every column starts at the same place in every row.
                if self.cols.show_author {
                    spans.push(author);
                }
                if self.cols.show_hash {
                    spans.push(Span::styled(
                        format!("{:<w$} ", k.short, w = HASH_W),
                        if self.compare.as_deref() == Some(k.oid.as_str()) {
                            Style::new()
                                .fg(theme.colors.accent.into())
                                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                        } else {
                            muted
                        },
                    ));
                }
                if self.cols.show_refs && self.refs_w > 0 {
                    let mut used = 0;
                    let refs =
                        pills(self.labels.get(&k.oid).map(Vec::as_slice).unwrap_or_default());
                    for (n, (r, remotes)) in refs.iter().enumerate() {
                        let w = ref_width(&theme.icons, r, remotes);
                        if used + w > self.refs_w {
                            // The rest does not fit: say how many are left.
                            let more = format!("+{}", refs.len() - n);
                            if used + more.len() <= self.refs_w {
                                used += more.len();
                                spans.push(Span::styled(more, muted));
                            }
                            break;
                        }
                        spans.extend(ref_pill(cx, r, remotes));
                        used += w;
                    }
                    spans.push(Span::raw(" ".repeat(self.refs_w.saturating_sub(used) + 1)));
                }
                if !self.cols.show_subject {
                } else if hit {
                    spans.push(Span::styled(
                        k.subject.clone(),
                        Style::new().fg(c.warning.into()).add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.extend(ui::subject::spans(cx, &k.subject, fg));
                }
                let mut when = Vec::new();
                if self.cols.show_absolute_time {
                    when.push(format!("{:<16}", pv::format_utc(k.time)));
                }
                if self.cols.show_relative_time {
                    when.push(format!("{:>8}", ago(k.time)));
                }
                (Line::from(spans), when.join("  "))
            }
            Focus::Submodules => {
                use git_core::SubmoduleState as St;
                let s = &self.submodules[i];
                let c = &theme.colors;
                let (mark, color) = match (s.state, s.dirty) {
                    (St::Conflict, _) => ("U", c.error),
                    (St::Uninitialized, _) => ("-", c.muted),
                    (St::Moved, _) => ("+", c.warning),
                    (_, true) => ("*", c.warning),
                    _ => (" ", c.muted),
                };
                (
                    Line::from(vec![
                        Span::styled(format!("{mark} "), Style::new().fg(color.into())),
                        Span::styled(s.path.clone(), fg),
                        Span::styled(format!("  {}", s.describe), muted),
                    ]),
                    s.oid.chars().take(8).collect(),
                )
            }
            _ => {
                let s = &self.stashes[i];
                (
                    Line::from(vec![
                        Span::styled(format!("{} ", s.index), muted),
                        Span::styled(s.message.clone(), fg),
                    ]),
                    ago(s.time),
                )
            }
        }
    }

    /// The graph cells of a row as coloured spans, padded to the width of the column.
    pub(in crate::app) fn graph_spans(
        &self,
        row: &git_core::GraphRow,
        cx: &ui::Context,
    ) -> Vec<Span<'static>> {
        let theme = cx.theme;
        let c = &theme.colors;
        let palette = [c.accent, c.success, c.warning, c.info, c.error, c.git.renamed];
        let mut spans: Vec<Span<'static>> = row
            .cells
            .iter()
            .take(self.graph_w)
            .map(|(ch, lane)| {
                Span::styled(ch.to_string(), Style::new().fg(palette[lane % palette.len()].into()))
            })
            .collect();
        let used = spans.len();
        spans.push(Span::raw(" ".repeat(self.graph_w.saturating_sub(used) + 1)));
        spans
    }

    /// The rev to preview for the highlighted entry of a list pane.
    pub(in crate::app) fn current_rev(&self, which: Focus) -> Option<String> {
        match which {
            Focus::Branches => self.current_ref().map(|b| b.name.clone()),
            Focus::Commits => match &self.commit_files {
                // The diff of the file (or folder) of the commit being explored: `oid`, a separator, the path.
                Some(cf) => Some(format!("{}\u{1}{}", cf.oid, cf.current_path().unwrap_or(""))),
                None if self.work => {
                    self.commits.get(self.lists[1].cursor()).map(|c| format!("{}\u{3}", c.oid))
                }
                None => self.commits.get(self.lists[1].cursor()).map(|c| match &self.compare {
                    // Two commits: the diff from the marked one to this one.
                    Some(base) if *base != c.oid => format!("{base}\u{2}{}", c.oid),
                    _ => c.oid.clone(),
                }),
            },
            Focus::Stashes => {
                self.stashes.get(self.lists[2].cursor()).map(|s| format!("stash@{{{}}}", s.index))
            }
            Focus::Submodules => {
                self.submodules.get(self.lists[3].cursor()).map(|s| format!("\u{4}{}", s.path))
            }
            Focus::Files => None,
        }
    }
}
