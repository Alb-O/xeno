# Tome

Kakoune-inspired modal text editor in Rust. Two crates:

- **tome-core**: Core library (input, motions, selections, extension system)
- **tome-term**: Terminal UI (ratatui, editor state, rendering)

## Extension System (`tome-core/src/ext/`)

Uses `linkme` for compile-time registration. Drop a file in, it's automatically included.

| Module | Purpose |
|--------|---------|
| `actions/` | Unified keybinding handlers returning `ActionResult` |
| `keybindings/` | Key → action mappings per mode |
| `commands/` | Ex-mode commands (`:write`, `:quit`) |
| `hooks/` | Event lifecycle observers |
| `options/` | Typed config settings |
| `statusline/` | Modular status bar segments |
| `filetypes/` | File type detection |
| `motions/` | Cursor movement |
| `objects/` | Text object selection |

Running cargo: `nix develop -c cargo {build/test/etc}`

## Agent Notes: GUI-Driven Debugging

- Approach: keep tight red/green loops with assertions in both unit tests and kitty GUI integration tests. Write failing assertions first, then iterate fixes in `tome-term` (movement/selection, multi-cursor insert) until GUI captures go green.
- Harness: exercise the real terminal path via `kitty-test-harness`, sending actual key sequences and capturing rendered screens. Keep tests serial and isolated per file to avoid socket/file contention.
- Why it matters: core selection ops can pass unit tests, but the live GUI harness exposes cursor/selection drift and per-cursor insert bugs. Running against the real terminal ensures fixes match user-facing behavior.