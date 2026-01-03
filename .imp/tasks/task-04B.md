# Evildoer: Typed Registry Handles - End-to-End Implementation

## Model Directive

Implement the typed handle system for registries as designed in task-04A analysis. This replaces stringly-typed internal coupling with compile-time safe typed handles. **This is an implementation task** - complete the full migration, not a partial solution.

______________________________________________________________________

## Implementation Expectations

\<mandatory_execution_requirements>

1. Implement changes incrementally, verifying each step compiles
1. Run `cargo check --workspace` after each major change
1. Run `cargo test --workspace` after completing the migration
1. Do not stop at partial completion - migrate ALL motion references
1. If you encounter architectural blockers, document them and propose solutions

Unacceptable:

- Leaving mixed string/typed APIs
- Breaking the build
- Partial migrations that leave inconsistent state

\</mandatory_execution_requirements>

______________________________________________________________________

## Design Specification

### Core Pattern: Typed Handles

```rust
/// A typed handle to a registry definition.
/// Zero-cost: just a pointer to a static.
#[derive(Copy, Clone)]
pub struct Key<T: 'static>(&'static T);

impl<T> Key<T> {
    pub const fn new(def: &'static T) -> Self { Self(def) }
    pub fn def(self) -> &'static T { self.0 }
}

// Per-registry type aliases
pub type MotionKey = Key<MotionDef>;
pub type PanelKey = Key<PanelDef>;
pub type CommandKey = Key<CommandDef>;
```

### Macro Changes

The `motion!` macro should generate:

1. The `MotionDef` static (as today, into distributed slice)
1. A public `MotionKey` constant for internal use

```rust
// Input
motion!(left, { description: "Move left" }, |text, range, count, extend| { ... });

// Generated output (conceptual)
#[linkme::distributed_slice(MOTIONS)]
static MOTION_left: MotionDef = MotionDef::new(...);

pub mod keys {
    pub const left: super::MotionKey = super::MotionKey::new(&super::MOTION_left);
}
```

### API Changes

```rust
// Before (string-based)
pub fn cursor_motion(ctx: &ActionContext, motion_name: &str) -> ActionResult

// After (typed)
pub fn cursor_motion(ctx: &ActionContext, motion: MotionKey) -> ActionResult

// Boundary lookup (for user input) remains string-based
pub fn find_motion(name: &str) -> Option<MotionKey>
```

### Usage Changes

```rust
// Before
action!(move_left, { ... }, |ctx| cursor_motion(ctx, "left"));

// After
use evildoer_registry_motions::keys as motions;
action!(move_left, { ... }, |ctx| cursor_motion(ctx, motions::left));
```

______________________________________________________________________

## Implementation Phases

### Phase 1: Add Key<T> Infrastructure

**Files:** `crates/registry/motions/src/lib.rs`

Tasks:

1. Define `Key<T>` struct with `Copy`, `Clone`, `Debug`
1. Define `MotionKey` type alias
1. Implement `Key::new()`, `Key::def()`, `Key::name()` (delegates to def)
1. Verify: `cargo check -p evildoer-registry-motions`

### Phase 2: Update motion! Macro

**Files:** `crates/registry/motions/src/macros.rs`

Tasks:

1. Modify `motion!` to generate a `keys` submodule with the constant
1. Handle the static visibility (needs to be accessible for Key::new)
1. Ensure distributed slice registration still works
1. Verify: `cargo check -p evildoer-registry-motions`

### Phase 3: Export Keys from Motions Crate

**Files:** `crates/registry/motions/src/lib.rs`, `crates/registry/motions/src/impls/*.rs`

Tasks:

1. Re-export the generated `keys` module at crate root
1. Verify all motion definitions generate their keys
1. Verify: `cargo check -p evildoer-registry-motions`

### Phase 4: Update Motion Helpers

**Files:** `crates/registry/actions/src/motion_helpers.rs`

Tasks:

1. Change `cursor_motion(ctx, motion_name: &str)` to `cursor_motion(ctx, motion: MotionKey)`
1. Change `selection_motion` similarly
1. Change `insert_with_motion` similarly
1. Update internal lookup logic (now just `motion.def()`)
1. Verify: `cargo check -p evildoer-registry-actions`

### Phase 5: Migrate Action Implementations

**Files:**

- `crates/registry/actions/src/impls/motions.rs`
- `crates/registry/actions/src/impls/insert.rs`
- Any other files using cursor_motion/selection_motion

Tasks:

1. Add `use evildoer_registry_motions::keys as motions;`
1. Replace all `cursor_motion(ctx, "left")` with `cursor_motion(ctx, motions::left)`
1. Replace all `selection_motion(ctx, "...")` similarly
1. Replace all `insert_with_motion(ctx, "...")` similarly
1. Verify: `cargo check --workspace`

### Phase 6: Update find_motion Return Type

**Files:** `crates/registry/motions/src/lib.rs`, consumers

Tasks:

1. Change `find_motion(name: &str) -> Option<&'static MotionDef>` to return `Option<MotionKey>`
1. Update all callers (likely in core/api crates)
1. Verify: `cargo check --workspace`

### Phase 7: Final Verification

Tasks:

1. `cargo test --workspace`
1. `cargo clippy --workspace`
1. Verify no string-based motion lookups remain in internal code (only at boundaries)

______________________________________________________________________

## Key Files Reference

```
crates/registry/motions/src/lib.rs          # MotionDef, MOTIONS slice, find_motion
crates/registry/motions/src/macros.rs       # motion! macro
crates/registry/motions/src/impls/basic.rs  # left, right, up, down
crates/registry/motions/src/impls/word.rs   # word motions
crates/registry/motions/src/impls/line.rs   # line_start, line_end
crates/registry/motions/src/impls/document.rs # document_start, document_end

crates/registry/actions/src/motion_helpers.rs    # cursor_motion, selection_motion
crates/registry/actions/src/impls/motions.rs     # move_left, move_right, etc.
crates/registry/actions/src/impls/insert.rs      # insert_line_start, etc.

crates/core/src/index/lookups.rs            # find_motion wrapper (if exists)
```

______________________________________________________________________

## Constraints

\<design_constraints>

- Key<T> must be Copy + Clone (zero-cost handle)
- Keys must be const-constructible (for static initialization)
- String lookup must remain available at boundaries (find_motion)
- No nightly features required
- Macro should work with existing motion! invocation syntax if possible

\</design_constraints>

\<scope_constraints>

- Focus on motions first (highest value, most string refs)
- Do NOT migrate panels/commands in this task (different architectural considerations)
- Do NOT change the crate topology yet (that's a separate task)
- Keep backward compatibility for find_motion at boundaries

\</scope_constraints>

______________________________________________________________________

## Edge Cases to Handle

1. **Motion aliases**: Some motions have aliases - keys should use the primary name
1. **Duplicate motion names**: If two crates define same motion name, keys would conflict - document this limitation
1. **Test code**: Tests that verify runtime lookup should continue using strings
1. **Scroll actions**: `crates/registry/actions/src/impls/scroll.rs` may use motions differently - check

______________________________________________________________________

## Success Criteria

- [ ] `Key<T>` infrastructure in place
- [ ] `motion!` macro generates keys
- [ ] `cursor_motion`, `selection_motion`, `insert_with_motion` accept `MotionKey`
- [ ] All internal motion references use typed keys
- [ ] String lookup (`find_motion`) still works for boundaries
- [ ] No nightly features required for this functionality
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` clean

______________________________________________________________________

## Anti-Patterns to Avoid

1. **Don't mix APIs**: If you change `cursor_motion` signature, change ALL callers
1. **Don't break incrementally**: Each phase should leave the build working
1. **Don't over-engineer**: Key<T> is just a pointer wrapper, keep it simple
1. **Don't forget Debug**: Keys should be debuggable (print the motion name)
