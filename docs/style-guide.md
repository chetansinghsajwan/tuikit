# Style guide

The source of truth for style in this repo. General rules live in the author's global agent
instructions; this file holds what is specific to tuikit.

## Tooling

Run everything through Taskfile tasks inside the dev shell (`nix develop`): `task fmt`,
`task clippy`, `task test`, `task scan`, `task check`. Hooks and CI call these tasks, never the raw
tools.

| Language       | Format    | Lint               |
| -------------- | --------- | ------------------ |
| GitHub Actions | yamlfmt   | actionlint, zizmor |
| Markdown       | dprint    | rumdl              |
| Nix            | alejandra | statix, deadnix    |
| Rust           | rustfmt   | clippy             |
| TOML           | taplo     | taplo              |
| YAML           | yamlfmt   | none exists        |

Spelling is checked with typos. Tests run with cargo-nextest. Scans: cargo-deny and osv-scanner
for dependencies, gitleaks for secrets, semgrep for SAST.

## Rust

- **Doc comments:** put a `///` doc comment on every function, type and module (`//!` at the top of
  the file), public or private, and on non-obvious constants. Say what it is for and why it
  exists, in simple language, not what each line does.
- **Tests:** a `#[test]` function whose name already says what it checks needs no doc comment.
- **Comments:** prefer `//` line comments over block comments.
- **Errors:** no `unwrap()` in production code; use `?`, or `expect` with the reason it cannot
  fail. Tests may unwrap.
- **Format:** `rustfmt.toml` sets the width (100) and heuristics; do not override it per file.
- **Module files:** a module that has a directory keeps its own file inside it as `mod.rs`
  (`app/mod.rs` next to `app/tests.rs`), never beside it as `app.rs`.
- **Names:** follow the `naming` skill (`~/.agents/skills/naming`): a count ends in `_count`, a size
  in `_size`, actions are verb phrases (`move_to_top`), callbacks start with `on_`.

## Commits

Conventional Commits, verified by `cog verify` in the commit-msg hook.

## Temp files

Use `.tmp/` in the repo root. It is git-ignored and excluded from the formatters and linters.
