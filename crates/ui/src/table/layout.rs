//! Columns: what a tool says about each one, and the one rule that turns them into widths for a given
//! row width, so the header and every row share the same layout.

use std::borrow::Cow;

/// Which side of its cell a column's text sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

/// How wide a column is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    /// Always this many cells.
    Fixed(u16),
    /// As wide as the widest cell (and title) seen so far, up to `max`. It only grows, so the columns do
    /// not jitter while the list scrolls; `Table::reset_widths` starts over.
    Fit { max: u16 },
    /// Shares what the other columns leave, by weight.
    Flex(u16),
}

/// What happens when the columns do not fit the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    /// Columns drop out, the highest `priority` first, until the rest fit (a viewer).
    #[default]
    Drop,
    /// Nothing drops; the table scrolls sideways to keep the cursor's column in view (an editor).
    Scroll,
}

/// What a column shows: the row's own cells, or a part the table draws from the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// The next of the row's `cells`.
    Cells,
    /// The row's `signs` (git letters), the most serious first.
    Signs,
    /// Whether the row is marked.
    Mark,
}

/// One column of a table: its id (what the sort names), its header title, how wide it is, which side
/// its text sits on and when it gives way.
#[derive(Debug, Clone)]
pub struct Column {
    pub id: &'static str,
    pub title: Cow<'static, str>,
    pub width: Width,
    pub align: Align,
    /// The fewest cells it is drawn in. For a flexible column it counts the part that shrinks (a name)
    /// only: the widest part beside it that does not shrink (guides, an icon, a count) is added.
    pub min: u16,
    /// When the columns do not fit (`Overflow::Drop`), the highest priority drops first; 0 never drops.
    pub priority: u8,
    /// The table can sort by it, starting in this direction (`true`: largest first).
    pub sort: Option<bool>,
    /// Its text is muted unless a cell says otherwise (sizes, times, counts).
    pub muted: bool,
    pub(crate) kind: Kind,
}

impl Column {
    /// A left-aligned column of the row's cells.
    pub fn new(id: &'static str, title: impl Into<Cow<'static, str>>, width: Width) -> Column {
        Column {
            id,
            title: title.into(),
            width,
            align: Align::Left,
            min: 0,
            priority: 0,
            sort: None,
            muted: false,
            kind: Kind::Cells,
        }
    }

    /// The one flexible column of a plain list: everything a row shows, the end giving way first.
    pub fn list() -> Column {
        Column::new("list", "", Width::Flex(1)).min(4)
    }

    /// Muted text at the right of a list (sizes, times, counts), as wide as the widest seen; the first
    /// to drop when the list is narrow.
    pub fn trailing() -> Column {
        Column::new("trailing", "", Width::Fit { max: 40 }).right().muted().priority(1)
    }

    /// The git letters at the right edge, the most serious first. The last cell is kept even when no
    /// row has a sign, so the columns beside it line up whether a row has one or not. Never dropped.
    pub fn signs() -> Column {
        Column {
            kind: Kind::Signs,
            align: Align::Right,
            min: 1,
            ..Column::new("signs", "", Width::Fit { max: 16 })
        }
    }

    /// The mark of a marked row (`Row::is_marked`).
    pub fn mark() -> Column {
        Column { kind: Kind::Mark, ..Column::new("mark", "", Width::Fixed(1)) }
    }

    /// Its text sits at the right of the cell (numbers, sizes).
    pub fn right(mut self) -> Column {
        self.align = Align::Right;
        self
    }

    pub fn min(mut self, cells: u16) -> Column {
        self.min = cells;
        self
    }

    pub fn priority(mut self, priority: u8) -> Column {
        self.priority = priority;
        self
    }

    /// The table can sort by it; `descending_first` is the direction it starts in.
    pub fn sortable(mut self, descending_first: bool) -> Column {
        self.sort = Some(descending_first);
        self
    }

    pub fn muted(mut self) -> Column {
        self.muted = true;
        self
    }
}

/// Where a column is drawn in a row: its offset from the row's left and its width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Placed {
    pub x: u16,
    pub width: u16,
}

/// The blank cells between two columns: the toolkit's one blank cell.
pub(crate) const GAP: usize = 1;

/// Where each of `columns` goes in a row `room` cells wide (`None`: not drawn), given the width each
/// wants (`wants`: the fixed or measured width, or a flexible column's minimum). With
/// `Overflow::Scroll` the columns before `first` are scrolled off to the left.
pub(crate) fn resolve(
    columns: &[Column],
    wants: &[usize],
    room: usize,
    overflow: Overflow,
    first: usize,
) -> Vec<Option<Placed>> {
    let mut width: Vec<usize> = wants.to_vec();
    let mut shown: Vec<bool> =
        (0..columns.len()).map(|i| overflow == Overflow::Drop || i >= first).collect();
    let total = |width: &[usize], shown: &[bool]| {
        let on: Vec<usize> = (0..columns.len()).filter(|&i| shown[i] && width[i] > 0).collect();
        on.iter().map(|&i| width[i]).sum::<usize>() + on.len().saturating_sub(1) * GAP
    };
    if overflow == Overflow::Drop {
        while total(&width, &shown) > room {
            // The highest priority drops first; of equal ones, the rightmost.
            let Some(drop) = (0..columns.len())
                .filter(|&i| shown[i] && columns[i].priority > 0)
                .max_by_key(|&i| (columns[i].priority, i))
            else {
                break;
            };
            shown[drop] = false;
        }
    }
    let flex: Vec<usize> = (0..columns.len())
        .filter(|&i| shown[i] && matches!(columns[i].width, Width::Flex(_)))
        .collect();
    let used = total(&width, &shown);
    if used < room {
        // The room left goes to the flexible columns by weight, the rest of a division to the last.
        let extra = room - used;
        let weights: usize = flex.iter().map(|&i| weight(&columns[i])).sum();
        let mut given = 0;
        for (n, &i) in flex.iter().enumerate() {
            let share = if n + 1 == flex.len() {
                extra - given
            } else {
                extra * weight(&columns[i]) / weights
            };
            // A flexible column that wanted nothing had no gap counted for it.
            let share = if width[i] == 0 && share > 0 { share.saturating_sub(GAP) } else { share };
            width[i] += share;
            given += share;
        }
    } else if used > room {
        // Still too wide: the flexible columns give up their minimum, then the right end is cut.
        let mut over = used - room;
        for &i in flex.iter().rev() {
            let take = over.min(width[i]);
            width[i] -= take;
            over -= take;
        }
    }
    let mut x = 0;
    let mut placed = vec![None; columns.len()];
    for i in 0..columns.len() {
        if !shown[i] || width[i] == 0 {
            continue;
        }
        if x >= room {
            break;
        }
        let w = width[i].min(room - x);
        placed[i] = Some(Placed { x: x as u16, width: w as u16 });
        x += w + GAP;
    }
    placed
}

/// The share a flexible column takes of the room left.
fn weight(column: &Column) -> usize {
    match column.width {
        Width::Flex(w) => usize::from(w.max(1)),
        _ => 0,
    }
}
