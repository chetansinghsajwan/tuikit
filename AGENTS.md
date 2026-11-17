# Agent instructions

- Follow [docs/style-guide.md](docs/style-guide.md) for style and tooling.
- Run `task check` before finishing a change; it formats-checks, lints and tests.
- Design rules are in [docs/ui-design.md](docs/ui-design.md) and
  [docs/usage-design.md](docs/usage-design.md).
- Use the dev shell (`nix develop`) for all tools.
- Always follow Conventional Commits (`type(scope): description`) for commit messages. The
  commit-msg hook verifies them with `cog verify`.
