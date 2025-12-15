## Code Style

- Prefer early returns over `else`; prefer `const` over `let mut` when possible
- Use `?` for error propagation; avoid `.unwrap()` in library code
- Prefer single-word variable names where unambiguous

## Build

`cargo build` compiles the project. `cargo test` runs tests. `nix build` produces a derivation. Use `nix develop` for the dev shell.

## Architecture

Two crates:
- `tome-core`: Core library with input handling, motions, text objects, and extension system
- `tome-term`: Terminal UI using ratatui, command execution, rendering

### Extension System (`tome-core/src/ext/`)

Uses `linkme` for zero-cost compile-time registration. New extensions are automatically included by adding to the distributed slices.

- **Actions** (`actions/`): Unified command/motion abstraction with string-based dispatch
  - `ActionDef`: Registers an action by name with a handler function
  - `ActionContext`/`ActionResult`: Context and results for action handlers
  
- **Keybindings** (`keybindings/`): Maps keys to actions per mode
  - `KeyBindingDef`: Registers a key -> action mapping with priority
  - New bindings checked first, falling back to legacy `keymap.rs`

- **Hooks** (`hooks/`): Event-driven lifecycle hooks
  - `HookDef`: Immutable event observers
  - `MutableHookDef`: Hooks that can modify editor state

- **Options** (`options/`): Typed configuration settings
  - `OptionDef`: Bool/Int/String settings with scope (global/buffer)

- **Motions** (`motions/`): Cursor movement functions
- **Objects** (`objects/`): Text object selection (word, quotes, etc.)
- **Commands** (`commands/`): Ex-mode commands (`:write`, `:quit`, etc.)
- **Filetypes** (`filetypes/`): Filetype detection and settings

### Legacy System

`keymap.rs` contains the original `Command` enum and static keymaps. The hybrid approach allows gradual migration to the new action system.

