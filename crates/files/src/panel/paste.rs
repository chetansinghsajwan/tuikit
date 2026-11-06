//! Pasting: the copy/move work, on a thread when it is big.

use super::*;

/// Bytes and items done so far by a background paste, and whether to stop.
pub(super) struct Progress {
    pub(super) done: AtomicU64,
    pub(super) total: u64,
    pub(super) items_done: AtomicUsize,
    pub(super) items: usize,
    pub(super) cancel: AtomicBool,
}

pub(super) struct PasteResult {
    pub(super) moved: Vec<(PathBuf, PathBuf)>,
    pub(super) done: usize,
    pub(super) failed: Vec<String>,
}

/// A paste running on its own thread.
pub(super) struct Job {
    pub(super) progress: Arc<Progress>,
    pub(super) handle: std::thread::JoinHandle<PasteResult>,
    pub(super) dir: PathBuf,
}

/// The work of a paste: copy or move each `(from, to)`, reporting through `progress`.
pub(super) fn run_paste(pairs: Vec<(PathBuf, PathBuf)>, cut: bool, progress: &Progress) -> PasteResult {
    let mut res = PasteResult { moved: Vec::new(), done: 0, failed: Vec::new() };
    for (src, dest) in pairs {
        if progress.cancel.load(Ordering::Relaxed) {
            res.failed.push("cancelled".into());
            break;
        }
        let mut tick = |n: u64| {
            progress.done.fetch_add(n, Ordering::Relaxed);
            !progress.cancel.load(Ordering::Relaxed)
        };
        let result = if cut { ops::move_progress(&src, &dest, &mut tick) } else { ops::copy_progress(&src, &dest, &mut tick) };
        let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match result {
            Ok(()) => {
                res.done += 1;
                if cut {
                    res.moved.push((src, dest));
                }
            }
            Err(e) => res.failed.push(format!("{name}: {e}")),
        }
        progress.items_done.fetch_add(1, Ordering::Relaxed);
    }
    res
}

/// Yanked (copy) or cut paths waiting for `p`.
#[derive(Clone)]
pub(super) struct Clip {
    pub(super) paths: Vec<PathBuf>,
    pub(super) cut: bool,
}

impl Panel {
    /// Paste the clipboard into the folder under the cursor. Small pastes happen at
    /// once; big ones (over `background_bytes`) run on a thread with a progress line
    /// in the footer, and Esc cancels them.
    pub(super) fn paste(&mut self) -> Action {
        if self.job.is_some() {
            self.say("a paste is already running (Esc cancels it)", true);
            return Action::None;
        }
        let Some(clip) = self.clip.take() else {
            self.say("nothing to paste (yank with y, cut with x)", true);
            return Action::None;
        };
        let dir = self.target_dir();
        // Work out every destination first, so two sources with one name do not collide.
        let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
        for src in &clip.paths {
            let Some(name) = src.file_name() else { continue };
            let dest = dir.join(name);
            if clip.cut && dest == *src {
                continue; // cut and pasted where it already is
            }
            let taken: Vec<PathBuf> = pairs.iter().map(|(_, d)| d.clone()).collect();
            let dest = ops::unique_among(dest, &taken);
            pairs.push((src.clone(), dest));
        }
        if !clip.cut {
            self.clip = Some(clip.clone()); // a yank can be pasted again
        }
        let total = ops::total_size(&pairs.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>());
        let progress = Arc::new(Progress { done: AtomicU64::new(0), total, items_done: AtomicUsize::new(0), items: pairs.len(), cancel: AtomicBool::new(false) });
        let cut = clip.cut;
        let work = {
            let progress = progress.clone();
            move || run_paste(pairs, cut, &progress)
        };
        if self.opts.background_bytes.is_some_and(|min| total >= min) {
            self.job = Some(Job { progress, handle: std::thread::spawn(work), dir });
            return Action::None;
        }
        self.finish_paste(work(), dir)
    }

    /// Apply a finished paste: keep folders open across moves, refresh, report.
    pub(super) fn finish_paste(&mut self, result: PasteResult, dir: PathBuf) -> Action {
        for (from, to) in &result.moved {
            self.remap_expanded(from, to);
        }
        self.expanded.insert(dir);
        self.refresh();
        if result.failed.is_empty() {
            self.say(format!("pasted {} item{}", result.done, if result.done == 1 { "" } else { "s" }), false);
        } else {
            self.say(result.failed.join("; "), true);
        }
        if result.moved.is_empty() { Action::None } else { Action::Moved(result.moved) }
    }

    /// If the background paste has ended, apply it; the action it produces waits in
    /// `pending` for the next event.
    pub(super) fn poll_job(&mut self) {
        if self.job.as_ref().is_some_and(|j| j.handle.is_finished()) {
            let job = self.job.take().expect("checked above");
            let result = job.handle.join().unwrap_or_else(|_| PasteResult { moved: Vec::new(), done: 0, failed: vec!["paste crashed".into()] });
            self.redraw = true;
            let action = self.finish_paste(result, job.dir);
            if action != Action::None {
                self.pending = Some(action);
            }
        }
    }

    /// The footer line while a paste runs: a percentage and the item count.
    pub(super) fn job_line(&self, theme: &Theme) -> Option<Line<'static>> {
        let j = self.job.as_ref()?;
        let p = &j.progress;
        let pct = if p.total == 0 { 0 } else { (p.done.load(Ordering::Relaxed) * 100 / p.total).min(100) };
        let text = format!("pasting {pct}% ({}/{}) - Esc cancels", p.items_done.load(Ordering::Relaxed).min(p.items), p.items);
        Some(Line::from(Span::styled(text, Style::new().fg(theme.colors.accent.into()))))
    }
}
