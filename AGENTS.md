# XENO

## Project Overview

Xeno is a modal text editor written in Rust, targeting terminals. It integrates Tree-sitter for syntax analysis, LSP IDE features. The architecture uses a distributed slice registry pattern via the `linkme` crate, allowing components (actions, commands, motions, text objects) to register themselves at compile time without centralized wiring.

## Build and Test

The project uses Nix flakes with `direnv`, though Agents should use `nix develop -c` directly. The Rust toolchain is pinned to `nightly-2026-01-01`.

```bash
cargo build                     # debug build
cargo build --release           # optimized build
cargo test                      # all tests
cargo test -p xeno-api          # single crate
cargo test buffer::tests        # specific test module
cargo insta review              # review snapshot test changes
nix flake check                 # format check, ast-grep lint, build
```

Integration tests using `kitty-test-harness` require a running Kitty terminal and won't execute in sandboxed builds.

## Code Style

The formatter enforces hard tabs, 100-character width, and module-level import granularity (`rustfmt.toml`). Three ast-grep rules in `lint/rules/` catch:

- Decorative comment banners (`// ====`, `// ----`): convert to `///` docstrings with `#` headers
- `#[allow(...)]` without `reason = "..."` justification
- Short inline comments under 25 chars: prefer self-documenting code or proper docstrings

Clippy allows `dbg!`, `print!`, `expect`, and `unwrap` in test code. Complexity thresholds: cognitive 25, args 8, type 250.

## Architecture

The workspace contains 25+ crates under `crates/`. The main binary lives in `crates/term` and produces the `xeno` executable.

**Core layers:** `xeno-base` defines fundamental types (Range, Selection, Key, Mode). `xeno-core` builds on these with ActionId, keymap resolution, and movement primitives. `xeno-api` exposes the Editor workspace, Buffer management, and UI state.

**Text representation:** Buffers use `ropey` for O(log n) text operations. Syntax highlighting flows from Tree-sitter via the `tree-house` abstraction, which loads grammar shared libraries at runtime from paths configured in KDL.

**Registry system:** The `crates/registry/` subtree contains 12 crates for extensible components. Each uses `#[distributed_slice]` to collect registrations: actions for keybindings, commands for `:` ex-mode, motions for cursor movement, text_objects for selection expansion, gutter for line decorations, statusline for status segments, hooks for lifecycle events. Adding a new action means annotating a function and it appears in the registry without touching dispatch code.

**LSP framework:** `xeno-lsp` implements a Tower-based async client that spawns language server processes, converts positions between byte/char/line representations, and routes diagnostics to the gutter and inline decorations.

**TUI:** `xeno-tui` is a modified Ratatui vendor. It renders to crossterm.

**Configuration:** KDL files parsed by `xeno-config`. Runtime assets (queries, themes, language configs) inside `crates/runtime/assets` embed via `xeno-runtime`.

## Implementation Patterns

### Registering Actions

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

Actions receive an immutable `ActionContext` containing `text: RopeSlice`, `cursor: CharIdx`, `selection: &Selection`, `count: usize`, and `args: ActionArgs`. They return `ActionResult::Effects(ActionEffects)` rather than mutating state directly. Effects compose via builder methods: `ActionEffects::motion(sel).with(Effect::SetMode(Mode::Insert))`.

### Edit Operations

Text mutations use the `EditOp` struct, a data-oriented description of pre-effects (yank, save undo), selection expansion, text transform, and post-effects (mode change). The `change` action returns `ActionEffects::edit_op(edit_op::change(true))` where `true` indicates "extend selection first if empty."

### Transaction Model

`Transaction` wraps a `ChangeSet` of retain/delete/insert operations with optional selection updates. The `ChangeSet::compose` method merges sequential edits, `invert` creates undo by swapping deletes with the original text, and `map_pos` transforms positions through changes with left/right bias. Undo snapshots clone the `Rope` into `Document.undo_stack`.

### Multi-Cursor Selection

`Selection` in `crates/base/src/selection.rs` holds a `SmallVec<[Range; 1]>` with a `primary_index`. The `transform` method applies a closure to all ranges, and `normalize` merges overlapping ranges. Adjacent ranges remain separate unless `merge_overlaps_and_adjacent` is called explicitly.

### Keymap Resolution

The keymap trie in `crates/keymap/src/matcher.rs` stores bindings in a hierarchical structure. Lookup returns `MatchResult::Complete`, `Partial { has_value }`, or `None`. Exact keys take precedence over character groups (`@digit`, `@upper`), which take precedence over wildcards (`@any`). The `continuations_with_kind` method powers which-key style UI by listing valid next keys at a prefix.

### Gutter Columns

Gutter columns register with the `gutter!` macro. Each defines a priority (lower = further left), width (fixed or dynamic via closure), and a render function receiving `GutterLineContext`:

```rust
gutter!(line_numbers, {
    description: "Absolute line numbers",
    priority: 0,
    width: Dynamic(|ctx| (ctx.total_lines.ilog10() as u16 + 1).max(3))
}, |ctx| {
    Some(GutterCell {
        text: format!("{}", ctx.line_idx + 1),
        style: if ctx.is_cursor_line { GutterStyle::Cursor } else { GutterStyle::Normal },
    })
});
```

Diagnostics flow from LSP through `GutterAnnotations` which carries `diagnostic_severity: u8` (0=none, 1=hint, 2=info, 3=warn, 4=error) and optional sign characters.

### Registry Collision Detection

All registries implement `RegistryMetadata` (id, name, priority, source). When multiple items share a name or keybinding, the one with higher priority wins. `crates/core/src/index/diagnostics.rs` collects collision reports for debugging.
