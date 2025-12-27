# Tome

Kakoune-inspired modal text editor in Rust.

## Design Goals

- **Orthogonal**: No tight coupling between modules, no dependency tangling. Event emitter/receiver pattern; emitters don't know what receivers may exist. Heavily utilize `linkme`'s `distributed_slices` for hierarchically inferred compile-time imports.
- **Suckless extension system**: Extensions are written in Rust, the same language as the editor's source code. A two-tier system (Core Builtins + Host Extensions) ensures the editor remains agnostic of specific features while allowing deep integration via TypeMaps.
- **Heavy proc macro usage**: Keeps repetitive data-oriented patterns lean and composable.

## Registry System

Uses `linkme` for compile-time registration. The registry system is split between **tome-manifest** (definitions and indexing) and **tome-stdlib** (actual implementations).

| Module           | Location                           | Purpose                                              |
| ---------------- | ---------------------------------- | ---------------------------------------------------- |
| `actions/`       | `crates/stdlib/src/actions/`       | Unified keybinding handlers returning `ActionResult` |
| `keybindings/`   | `crates/manifest/src/keybindings/` | Key → action mappings per mode                       |
| `commands/`      | `crates/stdlib/src/commands/`      | Ex-mode commands (`:write`, `:quit`)                 |
| `hooks/`         | `crates/stdlib/src/hooks/`         | Event lifecycle observers                            |
| `options/`       | `crates/stdlib/src/options/`       | Typed config settings                                |
| `statusline/`    | `crates/stdlib/src/statusline/`    | Modular status bar segments                          |
| `filetypes/`     | `crates/stdlib/src/filetypes/`     | File type detection                                  |
| `motions/`       | `crates/stdlib/src/motions/`       | Cursor movement                                      |
| `objects/`       | `crates/stdlib/src/objects/`       | Text object selection                                |
| `notifications/` | `crates/stdlib/src/notifications/` | UI notification system                               |

## Extension System (`crates/extensions/`)

Host-side extensions that manage stateful services (like ACP/AI) and UI panels. These are located in `crates/extensions/extensions/` and are automatically discovered at build-time via `build.rs`. They depend on `tome-api`.

Running cargo: `nix develop -c cargo {build/test/etc}`. Kitty GUI tests: `KITTY_TESTS=1 DISPLAY=:0 nix develop -c cargo test -p tome-term --test kitty_multiselect -- --nocapture --test-threads=1`.

## Integration & GUI-Driven Testing

- Approach: keep tight red/green loops with assertions in both unit tests and kitty GUI integration tests. Write failing assertions first, then iterate fixes until GUI captures go green.
- Harness: `kitty-test-harness` (git dependency, own flake) drives the real terminal, sending key sequences and capturing screens. Defaults favor WSL/kitty (X11, software GL). Current GUI suite lives in `crates/term/tests/kitty_multiselect.rs`; keep tests serial and isolated per file to avoid socket/file contention.
- Why it matters: core selection ops can pass unit tests, but the live GUI harness exposes cursor/selection drift and per-cursor insert bugs. Running against the real terminal ensures fixes match user-facing behavior.
