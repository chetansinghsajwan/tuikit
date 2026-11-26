//! `Table`: the one component for every scrolling set of rows (a file list, a process list, refs, pick
//! results, a CSV grid). A list is a table with one flexible column (`Column::list`) and no header.
//!
//! A tool keeps a `Table` between frames (the cursor, the scroll, the sort, the widths measured so far)
//! and draws it each frame with `Table::view`, passing the columns and a closure that builds a visible
//! row by its index. The data never moves into the table, so a million rows cost what ten do. The table
//! lays the columns out once for the header and every row, cuts what does not fit, scrolls, pins, draws
//! the tree guides and marks the cursor; a tool pads, aligns or highlights nothing itself.

/// The columns and how their widths are worked out.
mod layout;
/// What a row is made of, and how one is drawn.
mod row;
/// How the rows scroll.
mod scroll;
/// The state a tool keeps.
mod state;
/// One frame of a table.
mod view;

pub use layout::{Align, Column, Overflow, Width};
pub use row::{Cell, Row};
pub use state::{Sort, Table};
pub use view::TableView;

#[cfg(test)]
pub(crate) use row::Rows;

/// How many rows a `Table` keeps visible between the cursor and the edge while it scrolls (vim's
/// `scrolloff`) unless told otherwise (`Table::with_scroll_off`), so there is always context ahead of
/// the cursor.
pub const SCROLL_OFF: usize = 4;

/// Rows the user picked, by the tool's own key (a path, a name): kept next to the `Table`, as a mark
/// outlives the rows on screen (a file in a folded folder stays marked). The table draws a marked row
/// (`Row::is_marked`, shown in a `Column::mark`); this holds which rows are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marks<K: Ord> {
    keys: std::collections::BTreeSet<K>,
}

impl<K: Ord> Default for Marks<K> {
    fn default() -> Self {
        Marks { keys: std::collections::BTreeSet::new() }
    }
}

impl<K: Ord + Clone> Marks<K> {
    /// No rows marked.
    pub fn new() -> Marks<K> {
        Marks::default()
    }

    /// Marks `key`, or unmarks it if it was; true when it is now marked.
    pub fn toggle(&mut self, key: K) -> bool {
        if self.keys.remove(&key) {
            false
        } else {
            self.keys.insert(key);
            true
        }
    }

    /// Marks `key`.
    pub fn insert(&mut self, key: K) {
        self.keys.insert(key);
    }

    /// Unmarks `key`; true when it was marked.
    pub fn remove(&mut self, key: &K) -> bool {
        self.keys.remove(key)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.keys.contains(key)
    }

    pub fn clear(&mut self) {
        self.keys.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// The marked keys, in order.
    pub fn iter(&self) -> impl Iterator<Item = &K> {
        self.keys.iter()
    }

    /// What an action on "the marked rows, or the one under the cursor" works on: the marked keys, or
    /// `current` when none is marked.
    pub fn or_current(&self, current: Option<K>) -> Vec<K> {
        if self.keys.is_empty() {
            current.into_iter().collect()
        } else {
            self.keys.iter().cloned().collect()
        }
    }
}

/// Tests for the table.
#[cfg(test)]
mod tests;
