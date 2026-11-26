# Table design

Status: phase 1 built, 2026-11-26. Once both phases are built, the rules move into `ui-design.md` (Components, Scrolling) and this file goes.

## Decision

One interactive component, `ui::table::Table`, for every scrolling set of rows: a file list, a process list, refs, pick results, diagnostics, a CSV editor. A list is a table with one flexible column and no header. It replaces `ui::list::List`, `ListView`, the `left` and `right` spans of `ListRow`, and `ui::columns::Columns`.

The markdown table `pv` draws in a preview is not this component: it is a document element (wraps cells, counts the columns that do not fit) and keeps its own renderer, named "Markdown table" in `ui-design.md`.

## Rules

1. **The data stays in the tool.** The table never holds rows. Each frame it asks a closure for the visible rows only, so a million rows cost what ten do.
2. **The toolkit owns everything derived.** Column widths, alignment, cuts with `…`, the header, the cursor marks, scrolling, pinned rows, tree guides and the active scope, the sort marker and the empty text. A tool passes content and calls nothing else.
3. **Cells map to columns.** A row is a list of cells, one per column, so the header and every row share one layout and can not drift. A tool never pads a cell.
4. **One blank cell between columns,** always, the shrinking column gives way first (`ui-design.md`, Column padding).
5. **Immediate mode.** `Table` is the state a tool keeps between frames; the columns and the rows are passed each frame to `view`, which draws and is dropped.

## Parts

| Part        | Lives in                             | Holds                                                                                                    |
| ----------- | ------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| `Table`     | the tool's state, kept               | cursor (row, and column in cell mode), scroll offsets, scroll margin, sort, marked rows, measured widths |
| `Column`    | built by the tool, passed each frame | id, title, width rule, alignment, minimum width, drop priority, sortable                                 |
| `Row`       | the row closure, per visible row     | id and content (for the motion), tint, cells                                                             |
| `Cell`      | inside `Row`                         | styled spans                                                                                             |
| `TableView` | one frame                            | the options of this frame: header, pane, focus, empty text, pinned rows, tree depths                     |

## Sketch

```rust
pub struct Column {
    pub id: &'static str,
    pub title: Cow<'static, str>,
    pub width: Width,
    pub align: Align,
    /// Narrowest it is drawn; a flexible column under it is dropped instead.
    pub min: u16,
    /// Higher drops first when the columns do not fit (`Overflow::Drop`); 0 never drops.
    pub priority: u8,
    pub sortable: bool,
}

pub enum Width {
    Fixed(u16),
    /// As wide as the widest cell seen, up to `max`.
    Fit { max: u16 },
    /// Shares what is left by weight.
    Flex(u16),
}

pub enum Selection { Row, Cell }
pub enum Overflow { Drop, Scroll }

let mut table = Table::new();                     // kept by the tool
table.view(len, &columns, |i| Row { id, cells, ..Row::default() })
    .header(true)
    .pane(area)
    .focused(focused)
    .empty("nothing matches")
    .tree(|i| depth[i])                           // guides, active scope, pinned ancestors
    .render(cx, inner, buf);
```

`Column::list()` is the single flexible column of a plain list, `Column::signs()` the git letters at the right edge (fixed, never dropped, replaces `has_sign_cell`), `Column::mark()` the mark of a marked row.

## Layout

1. Fixed columns take their width; fit columns take the widest cell measured so far.
2. One blank cell goes between adjacent columns (`" │ "` instead when the separator is on).
3. If the rest is under the flexible columns' minimum:
   - `Overflow::Drop` (the default): drop the column with the highest priority, repeat. A list never overflows.
   - `Overflow::Scroll`: drop nothing; the flexible columns get their minimum and the table scrolls sideways.
4. The flexible columns share the rest by weight.

Fit widths are measured from the rows drawn and only grow, so the columns do not jitter while scrolling; `Table::reset_widths` starts over when the data is replaced (a new file).

## Behaviour

- **Rows.** Cursor, scroll margin (`SCROLL_OFF`, vim's `scrolloff`), rows the cursor skips (group headers, `set_rows`) and page moves work as `List` does today. `Table` is `Navigable`.
- **Header.** A pinned row drawn from the column titles, above the tree ancestors, never scrolled; the sort column accent and bold with `▼` or `▲`.
- **Tree.** `.tree(depth)` gives the depth of any row: the table draws the guides in the first flexible column, highlights the active scope and pins the ancestors of the top row. The tool computes neither.
- **Sort.** The table keeps the sort column and direction; `table.sort` moves to the next sortable column and `table.reverse` flips it. The tool reads `table.sort()` and sorts its data. One set of keys and one marker everywhere.
- **Marks.** A `ui::table::Marks<K>` kept next to the table holds the marked rows by the tool's own key (a path, a name), so a mark survives sorting, filtering and folding; the row says `is_marked` and the table draws it in `Column::mark`.
- **Cell mode** (`Selection::Cell`). The cursor has a column too; the cursor cell is the bright one and its row the faint one; left and right move it and the table scrolls sideways to keep it in view.
- **Motion.** Rows report themselves to `cx.motion` by id and content, as list rows do now.

The movement actions are the shell's (`Navigable`, `ui::app::run_nav`). Sorting and marking stay the tool's actions with their existing ids and keys (`sort.next`, `item.mark`), which call `Table::sort_next`, `reverse_sort` and `Marks::toggle`, so no user's key config breaks.

## Changes from the proposal

- **Marks live next to the table, not in it.** files and pick mark paths and strings that outlive the rows on screen (a file in a folded folder), and a row id can not be turned back into a path; `Marks<K>` keeps the tool's key instead.
- **No `table.*` actions.** The tools' own sort and mark ids stay, so key configs keep working.
- **A flexible column's minimum counts the part that shrinks.** The widest part of its cells that does not shrink (guides, icon, count) is added, which keeps the old rule that a name keeps four cells before the trailing text drops.
- **gu's commit list keeps its own columns inside one cell for now.** Its hash, graph, refs, author and time columns are worked out by gu (`fit_columns`); moving them into `Column`s is a follow-up.

## Phases

1. **Rows and columns.** Build `ui::table` (selection by row, `Overflow::Drop`, header, tree, sort, marks) and move every list to it: `files`, git panel, `gu`, `nu`, `pick`, `sm`, diagnostics, symbols. Remove `List`, `ListView`, `ListRow::left`/`right` and `Columns`. Bump helix-improved.
2. **Cells.** Add `Selection::Cell`, `Overflow::Scroll` and the separator, and move `tb`'s grid to it; `tb` keeps editing, undo, find and saving.

## Rejected

- **A `List` and a `Table`.** Two copies of scrolling, cursor and chrome that drift apart.
- **A table that owns its rows.** Copies the data, goes stale, fights immediate mode.
- **Free spans per row** (`left`, `right`). Alignment then depends on every tool padding alike.
- **Measuring every row for fit widths.** Costs the whole data each frame; a hint (`Fixed`) covers what must be exact.
