# tuikit

A terminal UI toolkit in Rust (on [ratatui](https://ratatui.rs)) and the tools built on it. One shared design, so every tool looks and behaves the same: colors, chrome, keys, menus.

## Tools

- **`pick`**: fuzzy picker for files, stdin, grep and git sources, with preview.
- **`pv`**: preview files in the terminal: code, Markdown, images, media, documents, tables, diffs.
- **`files`**: file explorer.
- **`gu`**: multi-pane git screen.
- **`tb`**: table editor (round-trips Markdown tables).
- **`nu`**: network connections (Wi-Fi, Bluetooth); NetworkManager and BlueZ on Linux, CoreWLAN and IOBluetooth on macOS.
- **`mdexport`**: Markdown to PDF through Typst.

## Crates

| Crate                        | What it is                                                   |
| ---------------------------- | ------------------------------------------------------------ |
| `tuikit-diagnostics-panel`   | Editor-independent diagnostics panel                         |
| `tuikit-files`               | File explorer panel                                          |
| `tuikit-git-core`            | Git status, diffs and history on gix                         |
| `tuikit-git-panel`           | Git changes panel with diff preview                          |
| `tuikit-nu-core`             | Connection backends for `nu`                                 |
| `tuikit-pick`                | Picker library (sources, matching, preview)                  |
| `tuikit-pv`                  | Preview and rendering library                                |
| `tuikit-symbols-panel`       | Editor-independent symbols panel                             |
| `tuikit-tb`                  | Table editing library                                        |
| `tuikit-ui`                  | Shared design: theme, keys, config, lists, prompts, animation |

Libraries live in `crates/`, the tools (`tuikit-<tool>` packages, binaries with the short names above) in `tools/`. In code the libraries keep their short names (`use ui::...`, `use pv::...`).

## Build

```sh
cargo build --workspace
cargo test --workspace
cargo run -p tuikit-pick-cli      # any tool: tuikit-<name>-cli, tuikit-gu, tuikit-nu, tuikit-mdexport
```

With Nix, every tool is a package of the flake: `nix build .#pick` (also `pv`, `files`, `gu`, `tb`, `nu`, `mdexport`).

## Design and plan

- [`docs/ui-design.md`](docs/ui-design.md): the look every tool follows.
- [`docs/usage-design.md`](docs/usage-design.md): keys, menus, shared verbs.
- [`plan.md`](plan.md): decisions and open questions.

## Used by

[helix-improved](https://github.com/chetansinghsajwan/helix-improved) embeds the panels and the picker by path.

## License

MIT OR Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
