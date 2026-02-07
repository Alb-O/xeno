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

Use full where clause syntax for readability:

```rust
// Correctimpl<T> CompletionModel for MyModel<T>where    T: HttpClientExt + Clone + Send + Debug + Default + 'static,{    // ...}// Avoid inline bounds for complex signaturesimpl<T: HttpClientExt + Clone + Send + Debug + Default + 'static> CompletionModel for MyModel<T> {
```

## Rustdoc

Always prefer comprehensive techspec docstrings over inline comments:
- If inline comment is spotted, consider merging it into docstring or removing if it's trivial
- Tests are more relaxed, but no need to state obvious flow

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

Every invariant must include all three fields as intra-doc links (see "Invariant Documentation Standard" below for full rules):

- Enforced in: `[crate::module::Type::method]` (absolute path, intra-doc link)
- Tested by: `[crate::module::invariants::test_*]` (link to proof wrapper)
- Failure symptom: concrete, user-visible or correctness symptom (plain text)

No triad field may be omitted. No `TODO` placeholders are permitted.

### Style rules

- No bold decorations in list items.
- Keep statements normative and checkable (must/should/may), and avoid vague prose.

### Maintenance rules

When changing core behavior, public API, or invariants in any subsystem:
1. Update the module-level rustdoc in the relevant anchor file in the same change set.
2. Add or update the `invariants::test_*` proof wrapper for the new invariant.
3. Verify with `./scripts/audit-anchors.sh`.

## Invariant Documentation Standard

### The Invariant Triad
Every architectural invariant must use the following format in module-level documentation:

```rust
//! - Must [invariant description]
//!   - Enforced in: [`crate::module::Type::method`]
//!   - Tested by: [`crate::module::invariants::test_name`]
//!   - Failure symptom: [concrete user-visible symptom]
```

No triad field may be omitted. No `TODO (add regression…)` placeholders are permitted in committed
invariants; every invariant must have an actual `invariants::test_*` link target.

### Machine-Checkable Links (rustdoc-audited)
- "Tested by" entries must be intra-doc links (`[...]`) targeting `invariants::test_*` items.
  - New/refactored code should collapse the proof logic directly into the `test_*` item using `#[cfg_attr(test, test)]` (or `tokio::test`) to avoid redundant `inv_*` wrappers.
- "Enforced in" entries must be intra-doc links where the target is linkable (pub/pub(crate) items in pub(crate)+ modules). Use backticks for truly private functions (e.g. private `fn` in a private module) or `#[cfg(test)]` test modules.
- Verify anchor links with:
  `RUSTDOCFLAGS="--document-private-items -D rustdoc::broken_intra_doc_links -A rustdoc::private_intra_doc_links -A warnings" cargo doc --workspace --no-deps`

### Absolute Path Rule for `//!` Anchor Docs
Because `//!` module-level docs resolve names from the parent scope (not the module's own scope),
**all intra-doc links in `//!` anchors must use absolute paths**:
- Same crate: `[crate::module::Type::method]`
- Different crate: `[xeno_lsp::module::Type]` (only if the crate is a dependency)
- Cross-crate enforcement sites in a non-dependent crate: use backticks with `(in xeno-editor)` note

Never use unqualified names, `self::`, or `super::` in anchor `//!` docs.

This rule applies to ALL `[links]` in `//!` docs - invariant triads, Key Types tables, Data Flow, etc.

### Anchor Module Visibility
Any module that contains an architecture anchor `//!` must be declared at least `pub(crate)` in its
parent `mod.rs` so that `crate::...` paths are resolvable. This is a visibility floor for anchor
pathability only; internal items remain private unless there is a separate API reason.

### The `invariants/` Module Pattern
Each anchor module must have a sibling `invariants` submodule directory with this structure:

1. In anchor `mod.rs`: `#[cfg(any(test, doc))] pub(crate) mod invariants;`
2. In `invariants/mod.rs`:
   - `pub(crate) mod catalog;`
   - curated `pub(crate) use catalog::{...};` exports for invariant constants
   - `#[cfg(doc)]` stubs for every `test_*` link target used in anchor docs
   - `#[cfg(test)] mod proofs;`
   - curated `#[cfg(test)] pub(crate) use proofs::{test_..., ...};` exports (no wildcard re-exports)
3. In `invariants/catalog.rs`: define each invariant as a named `pub(crate) const` with full triad rustdoc.
4. In `invariants/proofs.rs`: implement runnable `#[cfg_attr(test, test)]` or `#[cfg_attr(test, tokio::test)]` proof functions with `test_*` names.
5. Anchor docs must link to the stable wrapper path: `[crate::module::invariants::test_my_invariant]`.

Recommended mapping for discoverability:
- `MY_INVARIANT_NAME` (catalog constant) <-> `test_my_invariant_name` (proof/stub target)
