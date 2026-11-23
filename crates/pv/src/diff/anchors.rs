/// Where the files and the hunks start in a drawn patch (indexes into its lines), for jumping between them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Anchors {
    pub files: Vec<usize>,
    pub hunks: Vec<usize>,
    /// The path of each file in `files`.
    pub paths: Vec<String>,
}

impl Anchors {
    /// The first anchor after row `at` in `marks` (sorted), if any.
    pub fn next(marks: &[usize], at: usize) -> Option<usize> {
        marks.iter().copied().find(|&m| m > at)
    }

    /// The last anchor before row `at` in `marks` (sorted), if any.
    pub fn prev(marks: &[usize], at: usize) -> Option<usize> {
        marks.iter().copied().rev().find(|&m| m < at)
    }

    /// Everything shifted down by `n` rows (the patch sits under other lines).
    pub fn shifted(mut self, n: usize) -> Anchors {
        self.files.iter_mut().for_each(|m| *m += n);
        self.hunks.iter_mut().for_each(|m| *m += n);
        self
    }
}
