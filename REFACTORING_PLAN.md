# Xeno Editor Refactoring Plan

Comprehensive multi-phased plan for managing code scale and improving maintainability.

---

## Status Summary

| Phase | Description | Status |
|-------|-------------|--------|
| 0 | Guardrails and Observability | ✅ Complete |
| 1 | Extract UndoManager with Host Trait | ✅ Complete |
| 2 | Route Edit Push-Site Through UndoManager | ✅ Complete |
| 3 | Introduce EditExecutor | ✅ Complete |
| 4 | Effect Nesting Refactor | ✅ Complete |
| 5 | RegistryMeta Normalization | ✅ Complete |
| 6 | Capability Gating Consolidation | ✅ Complete |
| 7 | Editor Split (EditorCore extraction) | ✅ Complete |
| 7b | Move Capability Traits to EditorCore | ✅ Category A done |

**Current architecture is sound.** EditorCore extraction is paying off, crate boundaries
are respected, and the refactoring has not over-engineered the codebase.

---

## Architectural Constraints

**Registry/Editor crate boundary**: The registry crate (`xeno-registry`) defines
abstraction traits and cannot depend on the editor crate (`xeno-editor`).

**Rule**: Effects and capability traits → primitives types only, never editor types.

**Key constraint**: Effect interpreter logic must NOT migrate into EditorCore.
If you see `EditorCore::apply_effect(...)`, that's a code smell. Editor remains
the orchestration + policy layer.

---

## Recommended Next Steps

Based on analysis after completing Phases 0-7b:

### High Priority

1. **Split `move_visual_vertical` out of `EditAccess`**
   - Create `ViewAccess` or `MotionAccess` trait
   - Leave deprecated forwarding impl temporarily
   - Aligns trait semantics (it's a view operation, not an edit)

### Medium Priority

2. **Registry diagnostics hardening**
   - Add `--dump-registry` debug output behind a feature flag
   - Useful at 30+ crates for debugging weird dispatch

3. **ActionContext clone pressure**
   - The Rope clone in `execute_action` noted earlier
   - Consider lazy slicing or moving snapshot creation into executor layer

### Deferred (correctly)

- Moving focus state into EditorCore (high churn, low benefit)
- Splitting UndoHost further (Category B traits)
- Category C traits (fundamentally need Editor state)

---

## Completed Phases

### Phase 0: Guardrails and Observability

Established behavior locks before structural changes:
- Behavior-lock tests for undo/redo (cursor/selection/scroll restoration)
- Behavior-lock tests for effect ordering
- Debug logging for undo groups and effect dispatch

### Phase 1: Extract UndoManager with Host Trait

Isolated undo/redo logic into `UndoManager` with `UndoHost` trait:
- `UndoManager` owns undo/redo stacks
- `PreparedEdit` captures pre-edit state
- `UndoHost` trait for editor callbacks (notifications, document access)
- Editor implements `UndoHost` and delegates to `UndoManager`

### Phase 2: Route Edit Push-Site Through UndoManager

Centralized undo group creation:
- `apply_edit()` uses `prepare_edit()` / `finalize_edit()` pattern
- Redo stack only clears when group is actually pushed

### Phase 3: Introduce EditExecutor

Single entry point for edit operations:
- `EditExecutor<'a>` wraps `&'a mut Editor`
- `ApplyEditPolicy { undo, origin }` for policy control
- `apply_effects` uses `EditAccess` trait (correct cross-crate abstraction)

### Phase 4: Effect Nesting Refactor

Organized Effect variants by domain:
- Nested enums: `ViewEffect`, `EditEffect`, `UiEffect`, `AppEffect`
- `From` conversions for backward compatibility
- Builder API (`ActionEffects::*`) remains the stable public surface
- `#[non_exhaustive]` on Effect enum

### Phase 5: RegistryMeta Normalization

Reduced boilerplate across registry types:
- `RegistryMeta` struct with common fields (id, name, aliases, description, priority, etc.)
- `RegistryEntry` trait for introspection
- Updated `ActionDef`, `MotionDef`, `CommandDef`, `TextObjectDef`
- Specialized types (GutterDef, HookDef) kept using `impl_registry_metadata!`

### Phase 6: Capability Gating Consolidation

Single gate for all user-invoked operations:
- `Editor::run_invocation(Invocation)` checks capabilities
- `InvocationPolicy { enforce_caps, enforce_readonly }`
- Currently in log-only mode; flip to enforcing when ready
- Choke point lives in editor, not registry

### Phase 7: Editor Split (EditorCore Extraction)

Reduced Editor god-object pressure:

**EditorCore structure** (`crates/editor/src/impls/core.rs`):
```rust
pub struct EditorCore {
    pub buffers: BufferManager,
    pub workspace: Workspace,
    pub undo_manager: UndoManager,
}
```

**Editor** now contains:
- `core: EditorCore` - Core editing state
- UI: `ui`, `notifications`, `overlays`
- Layout: `layout`, `viewport`, `windows`, `focus`
- Config: `config`, `extensions`, `style_overlays`
- Integration: `hook_runtime`, `frame`, LSP fields

Compatibility accessors (`buffers()`, `workspace()`, `undo_manager()`) forward to core.

### Phase 7b: Capability Traits on EditorCore

Moved Category A (pure core) traits to EditorCore:
- `CursorAccess` - cursor(), cursor_line_col(), set_cursor()
- `SelectionAccess` - selection(), selection_mut(), set_selection()
- `MacroAccess` - record(), stop_recording(), play(), is_recording()
- `CommandQueueAccess` - queue_command()

Editor forwards to EditorCore for these traits.

**Category B** (needs focus): UndoAccess, JumpAccess, EditAccess - deferred
**Category C** (UI-coupled): ModeAccess, NotificationAccess, PaletteAccess, etc. - must stay on Editor

---

## Registry Safety

Watch for these issues:
1. **Duplicate IDs/aliases** - Collision detection via registry_diag commands
2. **Feature flags silently remove registrations** - CI feature-matrix builds essential
3. **Tests are separate binaries** - Registry contents can differ

Sanity test pattern:
```rust
#[test]
fn registry_sanity_check() {
    assert!(ACTIONS.len() >= 50);
    assert!(MOTIONS.len() >= 20);
}
```

---

## Summary Checklist

### Completed
- [x] Phase 0: Behavior-lock tests, debug logging
- [x] Phase 1: UndoManager, UndoHost trait, PreparedEdit
- [x] Phase 2: prepare/finalize pattern in apply_edit
- [x] Phase 3: EditExecutor, ApplyEditPolicy
- [x] Phase 4: Nested Effect enums, From conversions, builder API
- [x] Phase 5: RegistryMeta, RegistryEntry trait, updated macros
- [x] Phase 6: run_invocation, InvocationPolicy, log-only mode
- [x] Phase 7: EditorCore extraction, facade pattern
- [x] Phase 7b Category A: CursorAccess, SelectionAccess, MacroAccess, CommandQueueAccess on EditorCore

### Optional/Deferred
- [ ] Split `move_visual_vertical` into ViewAccess trait
- [ ] Registry diagnostics hardening (--dump-registry)
- [ ] ActionContext clone pressure optimization
- [ ] Phase 7b Category B: Move focus state to EditorCore
- [ ] Phase 7b Category B: Split UndoHost for EditorCore UndoAccess

---

*Generated from collaborative analysis with Claude and ChatGPT.*
