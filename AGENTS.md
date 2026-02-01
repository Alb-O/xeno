# XENO

## Project overview

Xeno is a TUI modal text editor written in Rust.
Key subsystems:
- Registry-backed definitions (actions/commands/motions/text objects/options/hooks/etc.)
- Tree-sitter syntax parsing/highlighting with tiered policy and background scheduling
- LSP client stack (JSON-RPC transport, doc sync, feature controllers)
- Unified overlay system for modal interactions + passive UI layers

## Build, test, format

Environment:
- Uses Nix flakes (`direnv` or `nix develop -c ...`)

Common commands:
- Format: `nix fmt`
- Test: `cargo test --workspace`
- Lint/check: `cargo check --workspace --all-targets`

## Rustdoc

Always prefer comprehensive techspec docstrings over inline comments:
- If inline comment is spotted, consider merging it into docstring or removing if it's trivial
- Tests are more relaxed, but no need to state obvious flow

## Architecture map (start here)

Module-level rustdoc is the ground truth for subsystem behavior and invariants, must read when relevant, keep updated:
- Registry: `crates/registry/src/core/index/runtime.rs`
- LSP: `crates/lsp/src/session/manager.rs`
- Broker: `crates/broker/broker/src/core/mod.rs`
- Overlay: `crates/editor/src/overlay/session.rs`
- Syntax: `crates/editor/src/syntax_manager/mod.rs`
- Windowing: `crates/editor/src/layout/manager.rs`

Workspace layout:
- `crates/term`: main binary (`xeno`)
- `crates/editor`: core editor implementation + overlay system
- `crates/registry`: definition indexing + runtime extension via snapshots
- `crates/lsp` + `crates/editor/src/lsp/*`: LSP framework + editor integration
- `crates/runtime/language`: tree-sitter integration and syntax primitives
- `crates/tui`: terminal UI backend (Ratatui-derived)
- `crates/runtime/config`: KDL config parsing
- `crates/runtime/data/assets`: embedded runtime assets (queries/themes/language configs)

## Architecture doc policy (module-level `//!` rustdoc)

Subsystem architecture lives as `//!` module-level rustdoc in the anchor files listed above.

### Required section order

1. Purpose
2. Mental model
3. Key types (table)
4. Invariants (hard rules)
5. Data flow
6. Lifecycle
7. Concurrency & ordering
8. Failure modes & recovery
9. Recipes

### Invariants contract (mandatory triad)

Every invariant MUST include all three fields:

- Enforced in: `Symbol::method` (qualified symbol name, no file paths)
- Tested by: `module::tests::test_*` OR `TODO (add regression: test_...)`
- Failure symptom: concrete, user-visible or correctness symptom

No invariant block is allowed to omit any of the triad fields.

### Enforcement site formatting

- Use qualified symbol names without file paths (findable via `rg`):
  - `OverlaySession::restore_all`
- For inline test references, include module context:
  - `core::tests::project_dedup_*`
- If multiple sites enforce the same rule, list all relevant symbols.

### Style rules

- No bold decorations in list items.
- Keep statements normative and checkable (MUST/SHOULD/MAY), and avoid vague prose.

### Maintenance rules

When changing core behavior, public API, or invariants in any subsystem:
1. Update the module-level rustdoc in the relevant anchor file in the same change set.
2. Add or update regression tests for the new invariant.
3. If a test is not practical immediately, add a `TODO (add regression: test_...)` entry.
