# UI design

How every terminal tool looks. Keys and behaviour are in `usage-design.md`. If a tool needs a value that is not here, add it here and to the `ui` crate first, then use it.

## Rules

1. **One source.** Colors, borders, spacing and sizes come from `~/.config/ui/theme.json` (written by Nix from stylix, read by the `ui` crate). A tool never hardcodes a color, a border or a size.
2. **One component per job.** Panes, lists, tabs, prompts, popups, tables and previews are drawn by `ui` and `pv`. A tool composes them and does not restyle them.
3. **Terminal only, truecolor.** Works on Linux, WSL and macOS, inside zellij.
4. **Nothing moves by itself.** No animation. Things change on a key, on focus, or when the data changes.

## Colors (roles, not hues)

| Role | Used for |
|---|---|
| `bg` `fg` | background and text |
| `muted` | secondary text: hints, units, line numbers, inactive borders |
| `accent` | focus, the current item, section names, links in the UI |
| `selection` | tinted backgrounds (code blocks, inline code) |
| `error` `warning` `success` `info` | messages, diagnostics, alerts. Never decoration. |
| `border` `border_focused` | pane borders: `muted` / `accent` |

Git: `added` green, `modified` yellow, `deleted` red, `renamed` blue, `untracked` cyan, `conflict` orange. One kind, one color, everywhere (rows, marks, diffs, minimap).

Syntax: `comment keyword function string number type constant variable operator`. One set (`home/programs/ui/syntax.nix`) feeds `pv` and the helix theme, so code looks the same in the editor and in a preview. A commit prefix (`fix:`) has one color per word (a hash of the word into 8 theme colors).

## Chrome

| Token | Default | Meaning |
|---|---|---|
| `chrome.border` | `rounded` | border glyphs (`rounded` `plain` `double` `thick`) |
| `chrome.titles` | on | pane titles |
| `chrome.transparent` | on | no solid background: the terminal's shows |
| `chrome.editor_box` | on | the editor sits in a bordered box like the docks |
| `layout.padding_x` / `padding_y` | 1 / 0 | inside a pane |
| `layout.gap` | 0 | between panes |

Every pane has a border and, when titles are on, a title with an icon. The focused pane's border is `border_focused`.

## Layout

| Token | Default | Meaning |
|---|---|---|
| `layout.sidebar_width` | 30 | left and right docks |
| `layout.bottom_height` | 12 | bottom dock |
| `layout.preview_percent` | 65 | the preview next to a list |
| `layout.popup_percent` | 90 | pickers and previews over the editor |

- A side dock is at most two fifths of the width; the editor keeps at least 20 columns.
- A focused files pane widens to fit its longest name (up to twice its width) and returns when it loses focus.
- A preview beside the files pane is a separate pane as tall as the editor pane. No title. The file picker's preview has none either (the list names the file); a commit preview keeps its title.
- A list and its preview sit side by side, the preview taking `preview_percent` of the width.
- The minimap is the rightmost strip of an editor view (10 columns, one blank column before it).

## Focus and dimming

| State | Look |
|---|---|
| focused pane | `border_focused` border, full brightness |
| other panes | dimmed to 62% (`dim::KEEP_INACTIVE`) |
| behind a popup | dimmed to 28% (`dim::KEEP`) |
| current row, pane focused | accent 28% over `bg`, bold |
| current row, pane not focused | accent 14% over `bg`, bold (the file open in the editor) |
| row with a git state | that color at 14% over `bg` |

## Components

| Component | Rule |
|---|---|
| Pane | border, title with icon, `padding_x` inside |
| List row | one line; icon only where it varies (file types), not on uniform rows; an indent guide `│` per level; marks and times on the right |
| Tabs | one strip on top, active tab accent and bold |
| Footer | one line: a prompt (`label value`), a question (`d again`), or the last message (`error` or `muted`) |
| Quick Keys | the leader popup: fixed 44 columns, centered, dims what is behind; an unknown key is named and the popup stays |
| Multi-select popup | tab strip of sections with counts, a list of `[x]` / `(•)` items, a `chosen` line, a key line |
| Table | bordered grid, header bold in `accent`, numbers right aligned, wide cells wrap, columns that do not fit are counted, never drawn |
| Code block | rounded box on a `selection`-tinted ground, the language as its first line inside |
| Alert | colored left bar and a title (`Note` `Tip` `Important` `Warning` `Caution`) |
| Diff | a tinted bar per file (fold mark, state letter, folder muted, name bold, counts at the right), a hunk line naming where it is (`⋯ line 48  fn name`), both line numbers, syntax colors, word-level marks, added and removed rows tinted, whitespace-only changes dimmed; lockfiles and huge files start folded; a line that does not fit ends in `…` |
| Commit preview | the subject first, the message under it, one muted line (author, age, hash, refs), then the files: folder muted, name in its state color, counts in columns, a bar for the size of the change |
| Minimap | braille dots, lines on screen tinted, cursor line, diagnostics and git changes in the edge column |

## Text

- Messages: short, lower case, no full stop (`staged`, `nothing staged to commit (stage with s)`).
- Questions end the line with the answer key: `Delete a.txt? d again`.
- Times: relative (`3d`), or `YYYY-MM-DD HH:MM` in UTC when absolute. Hashes: 8 characters.
- Tables and columns have fixed widths (one width per column, for every row); text that does not fit is cut with `…`, never wrapped into the next row. A flexible column (gu's refs) is sized from the widest row and takes the space freed when a column is hidden.
- Indent guides: the guide of the scope the cursor is in is highlighted in every tree view (`ui::tree::active_scope`, one call, no option), and code previews draw guides at each indent level (`ui::tree::mark_indent`). Staged changes are marked in the staged color (green), unstaged ones in the color of their change.
- Line counts (`+12 −3`) are aligned in columns: `ui::tree::CountWidths` sizes them from the whole list and `ui::tree::line_counts` lays out each row, so a tool only passes the numbers (gu's commit files and the commit preview use it).
- The name is the part that gives way: when a row is too narrow, the name of a file, folder or ref is cut with `…` and the columns, counts and signs beside it keep their place and alignment. A column that would leave the name under four cells is dropped instead. Widths are in cells (wide characters count double). This is built into `ui::tree::file_row`; a tool never trims by hand.
- What a cut column hides is shown whole elsewhere (the commit preview lists every ref).
- Empty state: one muted line saying why (`clean: nothing to commit`, `nothing matches`).

## Icons

- Nerd Font glyphs from `ui::icons`, colored per type.
- On pane titles, previews and list rows whose icon varies (files, folders, symbols). Not on rows that are all the same kind.

## Column padding

Wherever columns are drawn aligned (file rows, lists, tables, prompts beside counts), at least 1 blank cell separates adjacent columns, even when the row is narrow: the shrinking element (the name, the query, the message) gives way, never the gap.

## Motion

The toolkit owns animation (`ui::anim`); a tool never animates. The cursor row fades in (140 ms) when it lands on a new line, and `ui::anim::spinner()` replaces `…` while something runs. A tool's event loop calls `ui::anim::finish` after drawing, waits with `poll_timeout`, and redraws on `take_redraw`; a still screen costs no CPU. Config: `animations = "on" | "reduced" | "off"` (`reduced`: spinner only). Not built yet: eased scrolling (the scroll offset is each tool's today), fold reveal, popup fade.

## Adding something

1. Does `ui` or `pv` already draw it? Use it.
2. If not, add the component to `ui` with its colors from the theme and its sizes from the layout tokens. Add a test that draws it.
3. Add the rule to the table above in the same change.
