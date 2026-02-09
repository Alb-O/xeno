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

## Code style

Avoid inline bounds for complex signatures, use where clause:

```rust
impl<T> CompletionModel for MyModel<T>
where
    T: HttpClientExt + Clone + Send + Debug + Default + 'static,
{
    ...
}
```

## Rustdoc (& documentation)

- Always prefer comprehensive techspec docstrings over inline comments
- If inline comment is spotted, consider merging it into docstring or removing if it's trivial
- Tests are more relaxed, but no need to state obvious flow
- No bold decorations in list items (`**Prefix:** Actual description`) <- don't do this, be more concise with less formatting/decoration.

## Architecture map (start here)

Module-level rustdoc is the ground truth for subsystem behavior and invariants, must read when relevant, keep updated:
- Buffer: `crates/editor/src/buffer/mod.rs`
- Registry: `crates/registry/src/core/index/runtime.rs`
- LSP: `crates/lsp/src/session/manager/mod.rs`
- Overlay: `crates/editor/src/overlay/session.rs`
- Syntax: `crates/editor/src/syntax_manager/mod.rs`
- Windowing: `crates/editor/src/layout/manager.rs`

Workspace layout:
- `crates/term`: main binary (`xeno`)
- `crates/editor`: core editor implementation + overlay system
- `crates/registry`: definition indexing + runtime extension via snapshots
- `crates/lsp` + `crates/editor/src/lsp/*`: LSP framework + editor integration
- `crates/language`: tree-sitter integration and syntax primitives
- `crates/tui`: terminal UI backend (Ratatui-derived)

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

### Invariants

Anchor docs list invariants as brief one-line summaries, each `invariants.rs` test carries the full triad as a docstring:

```rust
/// Must [invariant description].
///
/// - Enforced in: `Type::method`
/// - Failure symptom: [concrete symptom]
#[cfg_attr(test, test)]
pub(crate) fn test_invariant_name() { ... }
```

No triad field may be omitted.

### Maintenance rules

When changing core behavior, public API, or invariants in any subsystem:
1. Update the module-level rustdoc in the relevant anchor file in the same change set.
2. Add or update the `invariants::test_*` proof in `invariants.rs` for the new invariant.
3. Verify with `./scripts/audit-anchors.sh`.

## Rustdoc link rules

### Absolute Path Rule for `//!` Anchor Docs
Because `//!` module-level docs resolve names from the parent scope (not the module's own scope),
**all intra-doc links in `//!` anchors must use absolute paths**:
- Same crate: `[crate::module::Type::method]`
- Different crate: `[xeno_lsp::module::Type]` (only if the crate is a dependency)
- Cross-crate enforcement sites in a non-dependent crate: use backticks with `(in xeno-editor)` note

Never use unqualified names, `self::`, or `super::` in anchor `//!` docs.

### The `invariants.rs` Module Pattern

Each anchor module has a sibling `invariants.rs` file:

1. In anchor `mod.rs`: `#[cfg(test)] mod invariants;`
2. In `invariants.rs`: implement `#[cfg_attr(test, test)]` (or `tokio::test`) proof functions
   with `test_*` names. Each test carries the full invariant triad as a docstring.
