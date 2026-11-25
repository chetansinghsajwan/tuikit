# Agent instructions

- Follow [docs/style-guide.md](docs/style-guide.md) for style and tooling.
- Run `task check` (format check and lint) and `task test` before finishing a change.
- Design rules are in [docs/ui-design.md](docs/ui-design.md) and
  [docs/usage-design.md](docs/usage-design.md).
- Use the dev shell (`nix develop`) for all tools.
- Agent worktrees live in `.claude/worktrees/`, which is git-ignored. Never commit anything from
  there, and keep it out of formatter and linter runs.
- Always follow Conventional Commits (`type(scope): description`) for commit messages. The
  commit-msg hook verifies them with `cog verify`.

## Architecture

- Pass the whole context, not one part of it. A function that draws takes `cx: &ui::Context` and
  reads `cx.theme` inside; it never takes `theme: &Theme`. The theme is one part of the context,
  next to the motion; passing the whole lets the context grow (focus, config, clock) without
  changing every signature on the way down. The same goes for any bundle: pass the owner, not
  the field you need today. This covers everything that draws into a buffer or builds what a
  frame shows (spans, lines, rows, layout splits). A pure lookup of a theme value (the color of a
  symbol kind, of a git change, of a load) stays a function of the theme, as the theme's own
  vocabulary.

  ```rust
  // Wrong: the theme is threaded on its own, and the next part of the context needs a new
  // parameter in every function between the screen and here.
  fn render_list(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
      let c = &theme.colors;
  }

  // Right: the context goes down whole.
  fn render_list(&mut self, cx: &ui::Context, area: Rect, buf: &mut Buffer) {
      let c = &cx.theme.colors;
  }
  ```

- State that every list, pane or component of a kind needs belongs to the toolkit's component,
  never to a copy in each tool. Before adding a field such as `cursor`, `offset` or `top` to a
  tool, check whether a `ui` component already owns it, and extend that component if it does
  not. A list is a `ui::list::List` the tool keeps and draws with `list.view(len, row)`; the tool
  passes each row's content, the list does the window, the scrolling, the pinned rows and the
  cursor marks.
