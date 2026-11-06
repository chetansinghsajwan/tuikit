//! The preview of the highlighted entry.

use super::super::*;

impl App {
    pub(in crate::app) fn render_preview(&mut self, which: Focus, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let rev = self.current_rev(which);
        let short = |s: &str| s.chars().take(8).collect::<String>();
        let whole = rev.clone().unwrap_or_default();
        let title = if let Some(a) = whole.strip_suffix('\u{3}') {
            format!("{}..work tree", short(a))
        } else if let Some((a, b)) = whole.split_once('\u{2}') {
            format!("{}..{}", short(a), short(b))
        } else if let Some((_, p)) = whole.split_once('\u{1}') {
            if p.is_empty() { "all files".to_string() } else { p.to_string() }
        } else {
            whole
        };
        let icon = match which {
            Focus::Branches => ui::icons::BRANCH,
            Focus::Stashes => ui::icons::STASH,
            _ => ui::icons::COMMIT,
        };
        let pane = Pane::new(theme).title(&title).icon(icon);
        let inner = pane.inner(area);
        pane.render(area, buf);
        let Some(rev) = rev else {
            self.preview = None;
            return;
        };
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let key = (which, rev.clone(), inner.width);
        let mut same_rev = false;
        if self.preview.as_ref().map(|p| &p.key) != Some(&key) {
            // A different commit starts with the files folded the way they fold by themselves.
            same_rev = self.fold_rev.as_deref() == Some(rev.as_str());
            if !same_rev {
                pv::diff_reset_folds();
                self.fold_rev = Some(rev.clone());
            }
            let opts = pv::Options { width: Some(inner.width), height: None, git: false, focus: None };
            let result = match rev.split_once('\u{1}') {
                _ if rev.ends_with('\u{3}') => {
                    let base = rev.trim_end_matches('\u{3}');
                    match Repo::discover(&self.root).ok_or_else(|| "not a repository".to_string()).and_then(|r| r.worktree_snapshot().map_err(|e| e.to_string())) {
                        Ok(snap) => pv::preview_range_marked(&self.root, Some(base), &snap, Some("work tree"), theme, &opts),
                        Err(e) => Err(pv::Error::Unsupported(e)),
                    }
                }
                _ if rev.contains('\u{2}') => {
                    let (base, oid) = rev.split_once('\u{2}').unwrap_or_default();
                    pv::preview_range_marked(&self.root, Some(base), oid, None, theme, &opts)
                }
                Some((oid, path)) => pv::preview_commit_files_marked(&self.root, oid, path, theme, &opts),
                None => pv::preview_commit_marked(&self.root, &rev, theme, &opts),
            };
            let (lines, anchors) = match result {
                Ok(r) => r,
                Err(e) => (vec![Line::from(Span::styled(e.to_string(), Style::new().fg(theme.colors.muted.into())))], Default::default()),
            };
            self.preview = Some(Preview { key, lines, anchors });
            if !same_rev {
                self.pscroll = 0;
            }
        }
        self.pheight = usize::from(inner.height);
        if let Some(p) = &self.preview {
            self.pscroll = self.pscroll.min(p.lines.len().saturating_sub(1));
            let shown: Vec<Line<'static>> = p.lines.iter().skip(self.pscroll).take(usize::from(inner.height)).cloned().collect();
            Paragraph::new(shown).render(inner, buf);
        }
    }
}
