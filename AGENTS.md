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

Agent docs are the ground truth for subsystem behavior and invariants, must read when relevant:
- Registry: `docs/agents/registry.md`
- LSP: `docs/agents/lsp.md`
- Broker: `docs/agents/broker.md`
- Overlay: `docs/agents/overlay.md`
- Syntax: `docs/agents/syntax.md`
- Windowing: `docs/agents/windowing.md`

Workspace layout:
- `crates/term`: main binary (`xeno`)
- `crates/editor`: core editor implementation + overlay system
- `crates/registry`: definition indexing + runtime extension via snapshots
- `crates/lsp` + `crates/editor/src/lsp/*`: LSP framework + editor integration
- `crates/runtime/language`: tree-sitter integration and syntax primitives
- `crates/tui`: terminal UI backend (Ratatui-derived)
- `crates/runtime/config`: KDL config parsing
- `crates/runtime/data/assets`: embedded runtime assets (queries/themes/language configs)

## Agent documentation policy (docs/agents/*.md)

All files in `docs/agents/` MUST follow the standard template and remain machine-navigable.

### Required section order (every file)

1. Purpose
2. Mental model
3. Module map
4. Key types (table)
5. Invariants (hard rules)
6. Data flow
7. Lifecycle
8. Concurrency & ordering
9. Failure modes & recovery
10. Recipes
11. Tests
12. Glossary

### Invariants contract (mandatory triad)

Every invariant MUST include all three fields:

- Enforced in: `<path>::<symbol>` (method/function-level granularity)
- Tested by: `<path>::test_*` OR `TODO (add regression: test_...)`
- Failure symptom: concrete, user-visible or correctness symptom

No invariant block is allowed to omit any of the triad fields.

### Tests section contract

- The Tests section MUST list concrete `test_*` functions (not just modules).
- If coverage is missing, use the standardized form:
  - `Tested by: TODO (add regression: test_<descriptive_name>)`

### Enforcement site formatting

- Prefer method/function granularity:
  - `crates/editor/src/overlay/session.rs`::`OverlaySession::restore_all`
- If multiple sites enforce the same rule, list all relevant symbols.

### Style rules

- No bold decorations in list items.
- Use consistent terminology from the Glossary.
- Keep statements normative and checkable (MUST/SHOULD/MAY), and avoid vague prose.

### Maintenance rules

When changing core behavior, public API, or invariants in any subsystem:
1. Update the relevant `docs/agents/<subsystem>.md` in the same change set.
2. Add or update regression tests for the new invariant.
3. If a test is not practical immediately, add a `TODO (add regression: test_...)` entry.

## Documentation updates

If you touch these areas, you MUST consult and update the matching agent doc:
- registry/indexing/override behavior → `docs/agents/registry.md`
- LSP identity/sync/generation/UI gating → `docs/agents/lsp.md`
- broker deduplication/routing/leases → `docs/agents/broker.md`
- overlay session/event wiring/focus rules → `docs/agents/overlay.md`
- syntax scheduling/hotness/tier policy/stale installs → `docs/agents/syntax.md`
- windowing/splits/floating windows/layers → `docs/agents/windowing.md`
