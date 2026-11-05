# Usage design

How every terminal tool behaves: keys, menus, safety, embedding. The look is in `ui-design.md`. If a tool needs a key or behaviour that is not here, decide it here first, then build it.

Applies to `files`, `tb` (table editor), `pick`, `pv` (where interactive), `gu`, the git panel, `nu`, the diagnostics and symbols panels, the helix docks and popups, and every future tool.

## Rules

1. **One grammar.** The same key means the same verb in every tool. A tool never uses another key for a verb in the table below.
2. **Discoverable.** Every action can be found by name (palette) and by key (`?`, Quick Keys). Hints on screen come from the same registry, so they cannot go stale.
3. **One engine.** `ui::keys` does dispatch, Quick Keys, the palette, help, hints, clash detection and user config. A tool declares its actions and implements them.
4. **Configurable once.** One user config (TOML) rebinds an action everywhere it appears.
5. **Embeddable.** In a host (a helix dock) the host's leader always reaches the host, unless the tool is taking typed text. Keys the tool does not claim go to the host.
6. **Safe.** Destructive actions ask first, and `d` again confirms. `Esc` always backs out.

## Finding actions

| What | Key | Notes |
|---|---|---|
| Quick Keys | `Space` | the leader popup: the tool's grouped actions, fixed size |
| Palette | `Space Space` | every action by name, with description and key; recently used first (20 kept per tool, `~/.local/state/ui/recent`, `UI_RECENT_FILE` moves it) |
| Keys help | `?` | the keys of the current pane, searchable |
| Unbound key | any | named in the footer (`space z is not bound`), the popup stays open |

- Single letters are for the shared verbs. Anything else tool-specific goes under the leader, so tools cannot clash.
- Helix keeps `:` as its own command line.
- A new tool gets recent-first by passing its name to `ui::keys::choose(tool, rows, title)`.

## Shared keys

| Key | Meaning | Where |
|---|---|---|
| `j` `k`, arrows, `ctrl-n` `ctrl-p` | move one | every list |
| `ctrl-j` `ctrl-k` | move five | everywhere `j` `k` move |
| `g` `G`, `home` `end` | top, bottom | every list |
| `ctrl-d` `ctrl-u` | scroll the preview half a page | wherever a preview shows |
| `Enter` | the primary action: open, accept, check out | all |
| `l` `h` | go in or expand, go out or collapse | trees (files, symbols, commit files, status) |
| `Tab` | mark for multi-select | files, pick |
| `/` | filter or search this list | all lists |
| `a` | add, new | files, branches, stashes |
| `d` | delete or discard (asks, `d` again confirms; `y` and `Enter` too) | all |
| `r` | rename | files, refs; reword in the commit list |
| `y` | copy (hash, path) | files, commits |
| `s` `u` `a` | stage or unstage (toggle), unstage, stage or unstage all | git |
| `1`-`9` | jump to a pane or tab | tools with tabs |
| `[` `]` | previous or next tab or panel | tools with tabs, docks |
| `Esc` | back out one level: prompt, confirmation, filter, focus to the list, marks | all |
| `q` | close the tool | standalone tools |
| `?` | keys of this pane | all |

In text, `ctrl-h` `ctrl-l` jump by words and stop at the end of a line first.

## Safety

- `d` deletes a file, branch, tag, stash or saved network, or discards changes. The footer asks (`Delete a.txt? d again`); `d`, `y` or `Enter` confirms, anything else cancels.
- Files go to the trash. Permanent delete is `D`, and asks the same way.
- Merge, rebase, reset hard, revert, force push and forget ask first.

## Tabs

One system for every tool (`ui::tabs`): open next to the active tab, close (the last tab stays), reopen (10 kept), next and previous with wrap, jump, move, duplicate.

| Key | Action |
|---|---|
| `Space t` `n` `c` | new, close |
| `Space t` `l` `h` | next, previous |
| `Space t` `r` `d` | reopen, duplicate |
| `Space t` `[` `]` | move left, right |
| `[` `]`, `1`-`9` | switch, jump (no numbers where buffers are the tabs: helix) |

When the labels do not fit, a window around the active tab shows, with `‹N` and `N›` for the tabs out of view.

## Live updates

Tools that show files or git state follow changes made outside them (`ui::watch`), without polling.

## Configuration

`~/.config/ui/config.toml` (shared, `UI_CONFIG`), then `~/.config/<tool>/config.toml` (`UI_TOOL_CONFIG`), which overrides it. A bad file is reported at startup and skipped.

```toml
leader = "space"
[keys]                          # action id -> key or keys; an empty list unbinds
"nav.down" = ["j", "down"]
"remote.push" = "leader r p"
```

Tool names: `gu`, `files`, `tb`, `nu`, `hx-ui` (the helix docks; not `helix`, which has its own config). `pick` and the diagnostics panel do not use the registry yet. Nix does not generate these files yet; `mdexport` is the exception (fonts from stylix).

## Embedding

- A panel in a host reports whether it is typing; then the leader is a character, not the leader.
- Keys a panel does not claim go to the host. Close and `Esc` return the keyboard to the editor.
- Helix docks: `Space 1` `2` `3` focus left, bottom, right. A file opened from elsewhere takes the keyboard back. The focused files pane widens for long names and shows a preview beside it.

## Standalone tools

- Flags where they apply: `--chooser-file`, `--cwd-file`, `--no-git`, `--no-preview`.
- Exit codes: 0 selected or done, 1 nothing matched, 2 usage or error, 130 cancelled.
- Draw on `/dev/tty`, so stdin and stdout stay free for pipes. Popup inside zellij.

## Tool specifics

| Tool | Where | Keys |
|---|---|---|
| `gu` | tabs | `1` Status, `2` Commits, `3` Refs, `4` stashes; `[` `]` |
| `gu` | Commits | `/` search, `f` filter popup, `F` clear, `v` view menu (`g` graph, `h` hash, `a` author, `n` name, `r` refs, `m` message, `t` time, `d` date, `b` all branches, `p` diff), `l` files of the commit, `m` mark a commit to compare (the preview then shows the diff from it to the highlighted commit; `m` again or `Esc` clears), `M` diff from the highlighted commit to the work tree (tracked files, uncommitted changes included), `enter` check out, `y` copy hash, `z` fold or unfold a file of the diff, `w` ignore whitespace-only changes, `r` reword, `A` amend, `R` revert, `leader S` switch to |
| `gu` | Status | tree of changes, `s` toggle stage, `a` all, `d` discard, `t` tree or list, `c` commit, `C` amend |
| `files` | all | `.` hidden, `I` ignored, `leader v t` tables as text, `leader f` find, `/` find in this folder |
| `tb` | grid | `h j k l` cell, `0` `$` first, last column, `enter` edit, `a` `O` add row, `d` delete row (asks), `J` `K` move row, `H` `L` move column, `leader c a` `leader c d` add, delete column, `s` `S` sort, `/` `n` `N` find, `y` `p` copy, paste, `u` `ctrl-r` undo, redo, `ctrl-s` save, `leader t h` header |
| `pick` | preview | `ctrl-t` tables as text |
| `pick` | grep (also helix `Space /`) | `alt-c` match case, `alt-w` whole word, `alt-r` regex or plain text (lit when on); `alt-i` include and `alt-e` exclude files by comma-separated globs (`*.rs, src/` and `target/, *.lock`), `enter` or `esc` back to the query; the search restarts on every change |
| helix | Space | `e` files, `m` Markdown and tables (`p` preview, `l` live, `s` source, `e` PDF; csv, tsv and psv are aligned in live mode, the default), `M` minimap, `V` dock preview, `T` table (`l` `h` `j` `k` cell, `s` `S` sort, `a` `d` add, delete column, `[` `]` move column; `Alt` `h` `j` `k` `l` move by cell), `1` `2` `3` docks |

## Checklist for a new tool

1. Declare actions in the registry: shared verbs first, the rest under the leader.
2. Every action has a name and a description (the registry test fails otherwise, and when two actions share a key).
3. `Esc` and `q` as above; destructive actions confirm.
4. Passes the host's leader through when embedded; reports typing.
5. Uses `ui` for rows, prompts, panes, popups and icons.
6. Standard flags and exit codes if standalone.
7. Add its row to *Tool specifics*.

## Code

| What | Where |
|---|---|
| keys engine, popup, palette, help | `ui::keys` |
| config | `ui::config` |
| tabs | `ui::tabs` |
| recent commands | `ui::recent` |
| file watching | `ui::watch` |
| choose-many popup, suggestions | `ui::multi`, `ui::prompt::Suggest` |
