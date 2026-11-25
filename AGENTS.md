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
  the field you need today.

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
  not.
