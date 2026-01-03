# Evildoer: Typed Registry Handles - Completion & Cleanup

## Model Directive

Complete the typed handle migration started in task-04B. Address remaining loose ends, add missing bindings, and consider extending the pattern to other registries.

______________________________________________________________________

## Context

Task-04B implemented the core typed handle system for motions:

- `Key<T>` / `MotionKey` infrastructure
- `motion!` macro generates `pub const` keys
- `cursor_motion()` etc. accept `MotionKey`
- Actions migrated to use `motions::left`, `motions::right`, etc.

## Remaining Work Identified

### 1. Screen Motions (runtime lookup)

**Location:** `crates/registry/actions/src/impls/motions.rs`

`move_top_screen`, `move_middle_screen`, `move_bottom_screen` use runtime lookup because the underlying motions (`screen_top`, `screen_middle`, `screen_bottom`) don't exist.

**Options:**

- A) Add viewport-aware motion definitions (requires access to viewport state)
- B) Remove these actions until viewport motions are implemented
- C) Keep runtime lookup as-is (current state)

**Decision needed:** These motions need viewport context that the current motion handler signature doesn't support. Either extend the motion system or defer these actions.

### 2. Added Actions Without Bindings

**Location:** `crates/registry/actions/src/impls/motions.rs`

`move_up` and `move_down` were added to satisfy registry tests but have no keybindings.

**Tasks:**

- Add bindings: `normal "j"` for down, `normal "k"` for up
- Or determine if `move_up_visual`/`move_down_visual` in scroll.rs are the intended actions for j/k

### 3. Scroll Actions

**Location:** `crates/registry/actions/src/impls/scroll.rs`

Check if scroll actions use string-based motion lookups that should be migrated.

### 4. Other String-Based Lookups

Audit for remaining string-based registry lookups in internal code:

- `ActionResult::TogglePanel("terminal")` / `"debug"` in window.rs
- `ActionResult::Command { name: "...", ... }` anywhere
- Any other `find_*("string")` calls in non-boundary code

______________________________________________________________________

## Implementation Phases

### Phase 1: Audit & Decide on Screen Motions

Tasks:

1. Check if viewport-aware motions are feasible with current architecture
1. Decide: implement, defer, or remove screen motion actions
1. If deferring, add TODO comments explaining the limitation

### Phase 2: Fix move_up/move_down Bindings

**Files:** `crates/registry/actions/src/impls/motions.rs`, `crates/registry/actions/src/impls/scroll.rs`

Tasks:

1. Determine relationship between `move_up`/`move_down` and `move_up_visual`/`move_down_visual`
1. Add appropriate bindings or consolidate duplicate actions
1. Verify j/k work correctly after changes

### Phase 3: Audit Scroll Actions

**Files:** `crates/registry/actions/src/impls/scroll.rs`

Tasks:

1. Check for string-based motion lookups
1. Migrate to typed keys if applicable

### Phase 4: Consider PanelKey / CommandKey

**Files:**

- `crates/registry/panels/src/lib.rs`
- `crates/registry/commands/src/lib.rs`
- `crates/registry/actions/src/impls/window.rs`

Tasks:

1. Assess value of typed handles for panels (cross-crate issue from task-04A)
1. Assess value of typed handles for commands
1. If worthwhile, implement following same pattern as MotionKey
1. If not worthwhile, document why and leave as-is

______________________________________________________________________

## Key Files

```
crates/registry/actions/src/impls/motions.rs   # screen motions, move_up/down
crates/registry/actions/src/impls/scroll.rs    # scroll actions
crates/registry/actions/src/impls/window.rs    # panel toggles (string-based)
crates/registry/motions/src/lib.rs             # motion registry
crates/registry/panels/src/lib.rs              # panel registry
```

______________________________________________________________________

## Success Criteria

- [ ] Screen motions either implemented, deferred with TODO, or removed
- [ ] move_up/move_down have correct bindings or are consolidated
- [ ] No string-based motion lookups remain in action implementations
- [ ] Decision documented for PanelKey/CommandKey (implement or defer)
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` clean
