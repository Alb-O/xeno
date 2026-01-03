# Xeno Registry Migration: Actions Registry

## Model Directive

Complete the registry-first architecture migration by moving the **actions** registry to `crates/registry/actions/`. This is the final and most complex registry, containing 87 actions with keybinding integration and a result dispatch system.

**Reference**: See `task-02A.md` for the overall migration pattern and completed registries.

**Scope**: Actions only. The hooks in `stdlib/src/hooks/` stay where they are (they were already migrated in task-02A).

______________________________________________________________________

## Implementation Expectations

\<mandatory_execution_requirements>

1. Edit files using tools to modify actual source files
1. Debug and fix by running builds, reading errors, iterating until it compiles
1. Run `cargo check --workspace` after major changes
1. Run `cargo test --workspace` after completing migration
1. Complete the full implementation; do not stop at partial solutions

Unacceptable:

- Providing code blocks without writing them to files
- Stopping after the first error
- Leaving the registry partially migrated

\</mandatory_execution_requirements>

______________________________________________________________________

## Behavioral Constraints

\<verbosity_and_scope_constraints>

- Match existing registry crate patterns (see `crates/registry/commands/`, `crates/registry/hooks/`)
- No inline comments narrating obvious control flow
- No decorative section markers
- Keep docstrings technical and rustdoc-compatible
- Update callsites directly - no re-export wrapper layers
- Remove old code after migration - no dead code

\</verbosity_and_scope_constraints>

\<design_freedom>

- The `DispatchResult` proc macro stays in `xeno-macro` but references must point to registry
- `parse_keybindings!` proc macro stays in `xeno-macro`
- Result handlers can move to the actions registry or stay with `editor_ctx` - use judgment based on dependencies
- `ActionContext` requires capabilities from `editor_ctx` - may need to stay in manifest or have careful dependency ordering

\</design_freedom>

______________________________________________________________________

## Current Architecture

### Files in manifest/src/actions/

- `mod.rs` - Module exports
- `context.rs` - `ActionContext`, `ActionArgs` (requires EditorCapabilities trait)
- `definition.rs` - `ActionDef`, `ActionHandler`
- `edit.rs` - `EditAction`, `ScrollAmount`, `ScrollDir`, `VisualDirection`
- `motion.rs` - `cursor_motion`, `selection_motion`, `insert_with_motion` helpers
- `pending.rs` - `PendingAction`, `PendingKind`, `ObjectSelectionKind`
- `result.rs` - `ActionResult` with `#[derive(DispatchResult)]`, `ActionMode`, handler slices

### Files in stdlib/src/actions/

- `mod.rs` - Module organization, `execute_action()` helper
- `editing.rs` - 21 editing actions (delete, change, yank, etc.)
- `find.rs` - 4 find/search actions
- `insert.rs` - 4 insert mode actions
- `misc.rs` - 3 misc actions (redo, etc.)
- `modes.rs` - 1 mode action
- `motions.rs` - 16 motion actions
- `scroll.rs` - 8 scroll actions
- `selection_ops.rs` - 14 selection actions
- `text_objects.rs` - 4 text object actions
- `window.rs` - 12 window/split actions

### Files in stdlib/src/editor_ctx/result_handlers/

- `mod.rs` - Module organization
- `core.rs` - Core result handlers (Ok, Error, Quit, etc.)
- `edit.rs` - Edit result handlers
- `mode.rs` - Mode change handlers
- `search.rs` - Search handlers

### Key Dependencies

- `ActionContext` uses `EditorCapabilities` trait from `manifest/src/editor_ctx/`
- `DispatchResult` proc macro generates handler slices referencing manifest types
- `parse_keybindings!` generates `KEYBINDINGS` slice entries
- Result handlers need `EditorContext` trait access

______________________________________________________________________

## Implementation Roadmap

### Phase 1: Create Actions Registry Structure

**1.1 Create `crates/registry/actions/Cargo.toml`**

```toml
[package]
name = "xeno-registry-actions"
description = "Actions registry for Xeno editor"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
xeno-base.workspace = true
xeno-registry-motions.workspace = true
xeno-macro.workspace = true
linkme.workspace = true
paste.workspace = true
```

**1.2 Create `crates/registry/actions/src/lib.rs`**

Move from `manifest/src/actions/`:

- `ActionDef`, `ActionHandler` (from definition.rs)
- `ActionMode` (from result.rs)
- `EditAction`, `ScrollAmount`, `ScrollDir`, `VisualDirection` (from edit.rs)
- `PendingAction`, `PendingKind`, `ObjectSelectionKind` (from pending.rs - already in base)
- `ACTIONS` distributed slice declaration
- Lookup functions: `find_action()`, `all_actions()`

Re-export `RegistrySource`, `Capability`, `flags` from motions.

**1.3 Create `crates/registry/actions/src/macros.rs`**

Move `action!` macro from `manifest/src/macros/actions.rs`.
Update all `$crate::` references to point to registry types.

**1.4 Create `crates/registry/actions/src/motion_helpers.rs`**

Move `cursor_motion`, `selection_motion`, `insert_with_motion` from `manifest/src/actions/motion.rs`.

______________________________________________________________________

### Phase 2: Handle ActionResult and Dispatch

**2.1 Decide on ActionResult location**

`ActionResult` uses `#[derive(DispatchResult)]` which generates handler slices. The proc macro references types via `xeno_manifest::`.

Options:
a) Move ActionResult to registry, update proc macro to reference `xeno_registry::actions::`
b) Keep ActionResult in manifest, only move ActionDef and implementations to registry

**Recommended: Option (a)** - Move ActionResult to registry for consistency.

**2.2 Update `crates/macro/src/dispatch.rs`**

Change generated code to reference `xeno_registry::actions::` instead of `xeno_manifest::`.

**2.3 Create `crates/registry/actions/src/result.rs`**

Move from `manifest/src/actions/result.rs`:

- `ActionMode` enum
- `ActionResult` enum with `#[derive(DispatchResult)]`
- `RESULT_EXTENSION_HANDLERS` slice

The `DispatchResult` derive will generate all `RESULT_*_HANDLERS` slices.

______________________________________________________________________

### Phase 3: Handle ActionContext

**3.1 Analyze ActionContext dependencies**

`ActionContext` in `manifest/src/actions/context.rs` uses:

- `EditorCapabilities` trait (from `manifest/src/editor_ctx/capabilities.rs`)
- Various capability traits

This creates a circular dependency if we move it to registry (registry can't depend on manifest).

**Solution**: Keep `ActionContext` in manifest. The registry defines `ActionDef` with handler signature `fn(&ActionContext) -> ActionResult`, but `ActionContext` is defined in manifest.

Update `ActionHandler` type alias to reference manifest's `ActionContext`:

```rust
pub type ActionHandler = fn(&xeno_manifest::ActionContext) -> ActionResult;
```

Or re-export `ActionContext` through the registry for ergonomics.

______________________________________________________________________

### Phase 4: Move Action Implementations

**4.1 Create `crates/registry/actions/src/impls/`**

Move all files from `stdlib/src/actions/`:

- `editing.rs` → `impls/editing.rs`
- `find.rs` → `impls/find.rs`
- `insert.rs` → `impls/insert.rs`
- `misc.rs` → `impls/misc.rs`
- `modes.rs` → `impls/modes.rs`
- `motions.rs` → `impls/motions.rs`
- `scroll.rs` → `impls/scroll.rs`
- `selection_ops.rs` → `impls/selection_ops.rs`
- `text_objects.rs` → `impls/text_objects.rs`
- `window.rs` → `impls/window.rs`

Update imports in each file to use registry types.

**4.2 Create `crates/registry/actions/src/impls/mod.rs`**

Module declarations for all implementation files.

______________________________________________________________________

### Phase 5: Handle Result Handlers

**5.1 Analyze result handler dependencies**

Result handlers in `stdlib/src/editor_ctx/result_handlers/` use:

- `EditorContext` trait for operations
- `HandleOutcome` enum
- Capability trait access

These have heavy dependencies on `manifest/src/editor_ctx/`.

**Solution**: Keep result handlers in stdlib/editor_ctx. They're consumers of the registry, not part of it.

**5.2 Update result handler imports**

Update `stdlib/src/editor_ctx/result_handlers/*.rs` to import:

- `ActionResult` from `xeno_registry::actions::`
- Handler slices from `xeno_registry::actions::`

______________________________________________________________________

### Phase 6: Wire Up Workspace

**6.1 Update root `Cargo.toml`**

Add to members:

```toml
"crates/registry/actions"
```

Add to workspace.dependencies:

```toml
xeno-registry-actions = { path = "crates/registry/actions" }
```

**6.2 Update `crates/registry/Cargo.toml`**

Add dependency:

```toml
xeno-registry-actions.workspace = true
```

**6.3 Update `crates/registry/src/lib.rs`**

Add re-exports:

```rust
pub use actions::{
    action, ActionDef, ActionHandler, ActionMode, ActionResult,
    EditAction, ScrollAmount, ScrollDir, VisualDirection,
    ACTIONS, dispatch_result,
    // ... all RESULT_*_HANDLERS slices
};
pub use xeno_registry_actions as actions;
```

______________________________________________________________________

### Phase 7: Update Manifest

**7.1 Slim down `crates/manifest/src/actions/`**

Keep only:

- `context.rs` - `ActionContext`, `ActionArgs` (depends on EditorCapabilities)
- `mod.rs` - Re-exports from registry + local context types

Remove:

- `definition.rs` (moved to registry)
- `edit.rs` (moved to registry)
- `motion.rs` (moved to registry)
- `pending.rs` (already in base)
- `result.rs` (moved to registry)

**7.2 Update `crates/manifest/src/lib.rs`**

Change re-exports to use registry:

```rust
pub use xeno_registry::actions::{
    action, ActionDef, ActionHandler, ActionMode, ActionResult,
    EditAction, ScrollAmount, ScrollDir, VisualDirection,
    ACTIONS, dispatch_result,
    // handler slices...
};
pub use actions::{ActionArgs, ActionContext}; // local types
```

**7.3 Add RegistryMetadata impl**

Create bridge impl for `ActionDef`:

```rust
impl crate::RegistryMetadata for xeno_registry::actions::ActionDef {
    // ... standard impl
}
```

______________________________________________________________________

### Phase 8: Update Stdlib

**8.1 Remove `crates/stdlib/src/actions/`**

Delete the entire directory (implementations moved to registry).

**8.2 Update `crates/stdlib/src/lib.rs`**

Remove `pub mod actions;`

**8.3 Update result handlers**

Update imports in `stdlib/src/editor_ctx/result_handlers/` to use registry paths.

______________________________________________________________________

### Phase 9: Update Proc Macro

**9.1 Update `crates/macro/src/dispatch.rs`**

Change all `xeno_manifest::` references to `xeno_registry::actions::`:

- Handler slice paths
- Type references (ActionResult, etc.)

**9.2 Verify keybinding macro**

`parse_keybindings!` in `crates/macro/src/keybindings.rs` generates entries for `KEYBINDINGS` slice.
Verify it still works with registry layout.

______________________________________________________________________

### Phase 10: Final Cleanup and Verification

**10.1 Remove dead code**

- Delete empty/unused files from manifest
- Remove any orphaned imports

**10.2 Build verification**

```bash
cargo check --workspace
```

**10.3 Test verification**

```bash
cargo test --workspace
```

**10.4 Clippy check**

```bash
cargo clippy --workspace
```

______________________________________________________________________

## Dependency Graph After Migration

```
xeno-base (Mode, PendingAction, Selection, etc.)
    ↓
xeno-registry-motions (RegistrySource, Capability, movement)
    ↓
xeno-registry-actions (ActionDef, ActionResult, action!, ACTIONS)
    ↓
xeno-registry (umbrella re-exports)
    ↓
xeno-manifest (ActionContext, EditorCapabilities, RegistryMetadata impls)
    ↓
xeno-stdlib (result_handlers in editor_ctx)
```

______________________________________________________________________

## Critical Considerations

### Circular Dependency Prevention

`ActionContext` requires `EditorCapabilities` which is defined in manifest. If we try to move `ActionContext` to registry, we create a cycle:

- Registry depends on base
- Manifest depends on registry
- Registry would need manifest for ActionContext

**Solution**: `ActionContext` stays in manifest. Registry defines the handler signature type but the concrete context type comes from manifest.

### Proc Macro Updates

The `DispatchResult` derive macro generates:

1. Handler slices (`RESULT_*_HANDLERS`)
1. `dispatch_result()` function
1. `is_terminal_safe()` method

All generated code references must be updated to `xeno_registry::actions::`.

### Keybindings Integration

The `action!` macro with `bindings:` field calls `parse_keybindings!` which generates `KEYBINDINGS` slice entries. This must continue to work after migration.

______________________________________________________________________

## Success Criteria

1. All 87 actions migrated to `crates/registry/actions/`
1. `cargo check --workspace` passes
1. `cargo test --workspace` passes
1. Action tests in `stdlib/src/actions/mod.rs` pass (or are moved)
1. Keybindings work correctly
1. Result dispatch works correctly
1. No duplicate type definitions
1. manifest/src/actions/ contains only `ActionContext` and re-exports

______________________________________________________________________

## Reference Files

Completed registry migrations to follow:

- `crates/registry/commands/src/lib.rs`
- `crates/registry/hooks/src/lib.rs`
- `crates/manifest/src/commands.rs` (RegistryMetadata impl pattern)

Files to migrate:

- `crates/manifest/src/actions/*.rs`
- `crates/manifest/src/macros/actions.rs`
- `crates/stdlib/src/actions/*.rs`
- `crates/macro/src/dispatch.rs` (proc macro to update)
