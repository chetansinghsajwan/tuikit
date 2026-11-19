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
