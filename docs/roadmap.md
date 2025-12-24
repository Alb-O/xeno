# Tome Improvement Task List

## Goals (as constraints)

- [ ] Preserve **orthogonal modules** (low coupling, clear boundaries)
- [ ] Keep **suckless-ish extensibility** (simple core, optional features)
- [ ] Maintain **drop-in registration ergonomics** (e.g. `linkme`), but with explicit validation
- [ ] Lean into **Rust superpowers** (proc-macros, types, property tests, invariants)

______________________________________________________________________

## Next 5 commits (highest leverage sequence)

### 1) Typed actions (`ActionId`) instead of stringly dispatch

- [x] Introduce `ActionId` (e.g. `u32` newtype or interned symbol)
- [x] Keep human-facing names at the edges (config/help), map to `ActionId` in registry build
- [x] Update input pipeline to emit `ActionId` (not `&'static str` / `String`)
- [x] Add a validation pass that rejects duplicate action names / IDs
  \- Collisions tracked in `RegistryIndex.collisions`
  \- Equal-priority collisions panic in debug builds
  \- Priority-based shadowing logged as warnings

### 2) Action metadata + capability enforcement at registry level

- [x] Extend action descriptors with `required_caps: &[Capability]`
  \- `ActionDef.required_caps` field in `tome-core/src/ext/actions/mod.rs`
  \- `CommandDef.required_caps` field in `tome-core/src/ext/mod.rs`
- [x] Registry build validates actions declare caps (and/or defaults)
  \- Test `test_no_unimplemented_capabilities` in `index.rs` validates no action uses unimplemented caps
- [x] Runtime dispatch checks caps once (before action fn runs)
  \- `check_all_capabilities()` in `EditorContext` (tome-core)
  \- Called in `execute_action` / `execute_command_line` (tome-term)
- [x] Decide policy for missing caps:
  - [x] Hard error: returns early with `MissingCapability` error shown to user
  - [ ] ~~Graceful no-op + status message~~ (rejected: hard error is clearer)

### 3) ChangeSet performance pass (avoid repeated `.chars().count()` hot paths)

- [x] Store cached char length in insert ops (`Insert { text, char_len }`)
  \- Added `Insertion` struct with `text: Tendril` and `char_len: CharLen` fields
  \- `Insertion::new()` computes length once at creation
  \- `Insertion::from_chars()` accepts pre-computed length for substrings
- [x] Refactor apply/map/compose paths to reuse cached lengths
  \- `apply()`: uses `ins.char_len` instead of `text.chars().count()`
  \- `map_pos()`: uses `ins.char_len` directly
  \- `invert()`: uses `ins.char_len` for delete count
  \- `compose()`: uses `ins.char_len` throughout, avoids repeated counting
- [ ] Add microbenchmarks for large inserts and repeated compositions

### 4) Selection primary stability + property tests

- [ ] Track selection primary by index or tagged marker (not equality search)
- [ ] Make normalization preserve “true primary” deterministically
- [ ] Add property tests for:
  - [ ] normalization idempotence
  - [ ] primary stability under sorting/merge/dedup
  - [ ] merge behavior when primary is inside merged spans

### 5) Registry & module boundaries (keep `linkme`, reduce implicit coupling)

- [ ] Create an explicit `RegistryBuilder` stage that consumes `linkme` slices and produces `Registry`
- [ ] Centralize validation in registry build:
  - [ ] Unique action names / IDs
  - [ ] Unique command names
  - [ ] Unique hook/event names
  - [ ] Verify all referenced actions exist
- [ ] Make the produced `Registry` immutable and passed explicitly (avoid global lookups)
- [ ] Add a debug tool:
  - [ ] Print all registered actions/commands/hooks in a deterministic order
  - [ ] Print origin/module info if available (feature-gated)

______________________________________________________________________

## Actions & dispatch (typed core, strings at edges)

- [ ] Introduce `ActionDescriptor { id, name, required_caps, handler, help, … }`
- [ ] Add a stable mapping layer:
  - [ ] `name -> ActionId`
  - [ ] `ActionId -> descriptor`
- [ ] Update keybinding resolution to return `ActionId`
- [ ] Add a “help” view that lists actions (name + description + caps)

______________________________________________________________________

## Capabilities system (strengthen what you already have)

- [ ] Make capabilities a first-class part of action metadata
- [ ] Remove ad-hoc scattered `require_*()` checks where possible:
  - [ ] Keep them for non-action internal code paths
  - [ ] Prefer centralized pre-dispatch checks for actions
- [ ] Add tests:
  - [ ] Action requiring `Search` fails/handles gracefully when absent
  - [ ] Action with no caps works in minimal context
- [ ] Consider adding “cap providers” per mode/context:
  - [ ] Editor provides: buffer, viewport, status
  - [ ] Optional: search, LSP, clipboard, filesystem, etc.

______________________________________________________________________

## Input handling (pipeline, fewer clones, clearer semantics)

### Make input a pipeline

- [ ] Refactor input processing into stages:
  - [ ] Stage 1: parse params (count/register/extend) from key stream
  - [ ] Stage 2: resolve binding for current mode + parsed modifiers
  - [ ] Stage 3: emit dispatch `{ action_id, params }`
- [ ] Add a small `InputState` struct to hold pending params cleanly
- [ ] Ensure reset logic is centralized (no scattered `reset_params()`)

### Reduce churn in command mode

- [ ] Stop passing `String` around on each keystroke
- [ ] Mutate `Mode::Command { input }` in place
- [ ] Add tests for backspace/escape/enter behavior in command mode

### Clarify “extend” semantics

- [ ] Decide if `Shift` implies extend always, or only in selection-aware contexts
- [ ] Consider representing extend as a modifier in bindings rather than “magic”
- [ ] Add tests:
  - [ ] Uppercase binding exists: it must win
  - [ ] No uppercase binding: fallback to lowercase (if desired)
  - [ ] Extend toggles behave consistently across modes

______________________________________________________________________

## Selection model (correctness + ergonomics + performance)

- [ ] Introduce a `SelectionBuilder`:
  - [ ] Collect ranges fast
  - [ ] Normalize once at end
- [ ] Fix primary tracking:
  - [ ] Use original index or explicit flag during normalization
  - [ ] Resolve primary when merging spans deterministically
- [ ] Add property tests:
  - [ ] Normalization idempotence
  - [ ] No overlaps after normalize/merge
  - [ ] Primary remains within selection set
- [ ] Add targeted unit tests:
  - [ ] Duplicate ranges with different primaries
  - [ ] Primary inside merged span
  - [ ] Adjacent merge rules

______________________________________________________________________

## ChangeSet / transactions (performance + invariants)

### Performance tasks

- [ ] Cache char lengths for inserts once (avoid repeated `.chars().count()`)
- [ ] Avoid repeated `chars().take(n).collect()` in compose:
  - [ ] Prefer slicing strategies or storing smaller normalized inserts
- [ ] Add benchmarks:
  - [ ] Large paste insert
  - [ ] Many small edits (typing)
  - [ ] Compose-heavy path (undo/redo merges)

### Correctness tasks

- [ ] Add invariants and tests:
  - [ ] `apply(invert(doc))` round-trips
  - [ ] `compose(a, b)` equals applying `a` then `b`
  - [ ] map_pos agrees with apply for sampled points
- [ ] Add fuzz/property tests for random edit sequences

______________________________________________________________________

## Testing strategy (fast feedback + deep correctness)

### Golden/snapshot tests

- [ ] Add render snapshots for key scenarios:
  - [ ] Empty buffer
  - [ ] Wrapped lines
  - [ ] Selection highlight
  - [ ] Multiple cursors
- [ ] Add deterministic output formatting (stable ordering, stable widths)

### Property / fuzz tests

- [ ] Selection invariants (see Selection section)
- [ ] ChangeSet invariants (see ChangeSet section)
- [ ] Input pipeline invariants:
  - [ ] Params reset after action emission
  - [ ] Count parsing correctness (e.g. `0`, leading zeros policy)

### Integration harness

- [ ] Expand kitty test harness coverage:
  - [ ] Basic navigation and editing flows
  - [ ] Undo/redo flows
  - [ ] Command mode entry/exit flows

______________________________________________________________________

## Optional “nice-to-have” follow-ups

- [ ] Add a `--dump-registry` CLI for introspection
- [ ] Add a `--check-config` CLI to validate user keybindings against registry
- [ ] Add docs:
  - [ ] “How to write an action” (proc-macro usage)
  - [ ] “How keybinding resolution works” (pipeline)
