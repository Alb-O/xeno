# XENO

a modal text editor, helix/kakoune-like.

subsystems:
* registry-backed defs (actions/commands/motions/text objects/options/hooks/etc.)
* tree-sitter syntax parsing/highlighting with tiered policy and background scheduling
* lsp client stack (JSON-RPC transport, doc sync, feature controllers)
* unified overlay system for modal interactions + passive UI layers

## build, test, format

environment: nix flake (`direnv` or `nix develop -c ...`)

common commands:
* format: `nix fmt`
* test: `cargo test --workspace`
* lint/check: `cargo check --workspace --all-targets`

## code style

use `where` clause:

```rust
impl<T> CompletionModel for MyModel<T>
where
    T: HttpClientExt + Clone + Send + Debug + Default + 'static,
{
    ...
}
```

## rustdoc (& documentation)

* prefer comprehensive techspec docstrings over inline comments
* if inline comment is spotted, consider merging it into docstring or removing if it's trivial
* tests are more relaxed, but no need to state obvious flow
* no bold decorations in list items, e.g `**Prefix:** Actual description` <- don't do this shit, be more concise with less formatting/decoration
* use `*` instead of `-` for bullet points

## git commit style

* conventional, two `-m`s; header and detailed bulleted body.
* escape backticks (use single quotes in bash)

## architecture map (start here)

module-level rustdoc is the ground truth for subsystem behavior and invariants, must read when relevant and keep updated.

get the filepath and read the file in one:
* buffer: `rg -N -C999 --glob '!AGENTS.md' "XENO_ANCHOR_BUFFER"`
* registry: `rg -N -C999 --glob '!AGENTS.md' "XENO_ANCHOR_REGISTRY_RUNTIME"`
* lsp: `rg -N -C999 --glob '!AGENTS.md' "XENO_ANCHOR_LSP_MANAGER"`
* overlay: `rg -N -C999 --glob '!AGENTS.md' "XENO_ANCHOR_OVERLAY_SESSION"`
* syntax: `rg -N -C999 --glob '!AGENTS.md' "XENO_ANCHOR_SYNTAX_MANAGER"`
* windowing: `rg -N -C999 --glob '!AGENTS.md' "XENO_ANCHOR_LAYOUT_MANAGER"`

## architecture docs

subsystem architecture lives as `//!` module-level rustdoc in the anchor files listed above.

### required sections

* purpose
* mental model
* key types (table)
* invariants
* data flow
* lifecycle
* concurrency & ordering
* failure modes & recovery
* recipes

### invariants

each anchor module has an `invariants.rs` file:
* in anchor `mod.rs`: `#[cfg(test)] mod invariants;`
* in `invariants.rs`: implement `#[cfg_attr(test, test)]` (or `tokio::test`) proof functions with `test_*` names. each test carries the full invariant triad as a docstring.

anchor docs list invariants as brief one-line summaries, each `invariants.rs` test carries the full triad as a docstring:

```rust
/// Must [invariant description].
///
/// * Enforced in: `Type::method`
/// * Failure symptom: [concrete symptom]
#[cfg_attr(test, test)]
pub(crate) fn test_invariant_name() { ... }
```

when changing core behavior, public API, or invariants in any subsystem:
* update the module-level rustdoc in the relevant anchor file in the same change set.
* add or update the `invariants::test_*` proof in `invariants.rs` for the new invariant.
