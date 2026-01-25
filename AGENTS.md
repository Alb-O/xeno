# XENO

## Project Overview

Xeno is a TUI modal text editor written in Rust. Tree-sitter for syntax analysis, LSP IDE features. The architecture uses an registry pattern via the `inventory` crate, allowing components (actions, commands, motions, text objects) to register themselves at compile time without centralized wiring.

## Build and Test

The project uses Nix flakes with `direnv` or `nix develop -c` directly.

## Architecture

The workspace contains many crates under `crates/`. The main binary lives in `crates/term` and produces the `xeno` executable.

`xeno-primitives` defines fundamental types (Range, Selection, Key, Mode). `xeno-editor` builds on these with Editor workspace, Buffer management, Document state, and UI coordination. `xeno-keymap-core` handles keymap resolution and input matching. Buffers use `ropey` for O(log n) text operations. Syntax highlighting flows from Tree-sitter via the `tree-house` abstraction, which loads grammar shared libraries at runtime from paths configured in KDL.

The `crates/registry/` subtree contains many sub-crates for extensible components. Each uses `inventory` to collect registrations: actions for keybindings, commands for `:` ex-mode, motions for cursor movement, textobj for selection expansion, gutter for line decorations, statusline for status segments, hooks for lifecycle events. Adding a new action means annotating a function and it appears in the registry without touching dispatch code.

`xeno-lsp` implements the LSP client stack. See [docs/agents/lsp.md](docs/agents/lsp.md) for architecture details.

`xeno-tui` is a modified Ratatui vendor. It renders to crossterm.

KDL files parsed by `xeno-runtime-config`. Runtime assets (queries, themes, language configs) inside `crates/runtime/data/assets` embed via `xeno-runtime-data`.

Actions use the `action!` macro in `crates/registry/actions/src/macros.rs`. The macro accepts inline keybindings in KDL syntax and registers via `#[distributed_slice]`:

```rust
action!(move_left, {
    description: "Move cursor left",
    bindings: r#"normal "h" "left"
insert "left""#,
}, |ctx| cursor_motion(ctx, motions::left));

action!(document_end, {
    description: "Goto file end",
    bindings: r#"normal "g e" "G""#,
}, |ctx| cursor_motion(ctx, motions::document_end));
```

Actions receive an immutable `ActionContext` containing `text: RopeSlice`, `cursor: CharIdx`, `selection: &Selection`, `count: usize`, and `args: ActionArgs`. They return `ActionResult::Effects(ActionEffects)` rather than mutating state directly. Effects use nested enums for type safety: `ActionEffects::motion(sel).with(Effect::App(AppEffect::SetMode(Mode::Insert)))`.

Text mutations use the `EditOp` struct, a data-oriented description of pre-effects (yank, save undo), selection expansion, text transform, and post-effects (mode change). The `change` action returns `ActionEffects::edit_op(edit_op::change(true))` where `true` indicates "extend selection first if empty."

`Transaction` wraps a `ChangeSet` of retain/delete/insert operations with optional selection updates. The `ChangeSet::compose` method merges sequential edits, `invert` creates undo by swapping deletes with the original text, and `map_pos` transforms positions through changes with left/right bias. Undo uses a pluggable `UndoBackend` with two strategies: `SnapshotUndoStore` (full rope snapshots) or `TxnUndoStore` (transaction pairs via `invert`).

The keymap trie in `crates/keymap/core/src/matcher.rs` stores bindings in a hierarchical structure. Lookup returns `MatchResult::Complete`, `Partial { has_value }`, or `None`. Exact keys take precedence over character groups (`@digit`, `@upper`), which take precedence over wildcards (`@any`). The `continuations_with_kind` method powers which-key style UI by listing valid next keys at a prefix.