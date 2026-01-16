# Xeno Editor Refactoring Plan

Comprehensive multi-phased plan for managing code scale and improving maintainability.
Synthesized from architectural analysis of the 24+ crate workspace.

---

## Executive Summary

The Xeno editor has grown to a substantial codebase with a distributed slice registry pattern
for extensibility. This plan addresses:

1. **Editor god-object pressure** - Editor owns too many concerns
2. **Undo/transaction complexity** - Two overlapping history layers need isolation
3. **Effect interpreter sprawl** - 24 Effect variants growing toward "second Editor"
4. **Registry boilerplate** - Common metadata repeated across all registry types
5. **Capability enforcement gaps** - Multiple entry points with inconsistent gating

---

## Architectural Constraints

**Registry/Editor crate boundary**: The registry crate (`xeno-registry`) defines
abstraction traits and cannot depend on the editor crate (`xeno-editor`). This
prevents circular dependencies but means:

- Effects and capability traits must not mention editor-specific types
- They can freely use primitives types (`EditOp`, `Selection`, `Range`, `Mode`)
- New types crossing action/effect/capability boundaries must live in primitives or registry

**Rule**: Effects and capability traits → primitives types only, never editor types.

**Implications for each phase**:
- Phase 4: Keep effects high-level (EditOp, Paste), not low-level (Transaction, EditPlan)
- Phase 5: RegistryMeta lives in registry; editor never needs to know about it
- Phase 6: Choke point lives in editor, not registry (registry stays declarative)
- Phase 7: Trait impls move to EditorCore, but trait definitions stay in registry

---

## Phase 0: Guardrails and Observability (Pre-refactor Safety)

**Purpose**: Establish behavior locks before making structural changes.

### Tasks

- [x] Add behavior-lock tests for undo/redo:
  - [x] Undo/redo restores cursor/selection/scroll for same-doc multiple buffers
  - [x] Redo stack clears on new edit
  - [x] `MergeWithCurrentGroup` coalescing around `insert_undo_active()`
- [x] Add behavior-lock tests for effect interpreter:
  - [x] Effect ordering: cursor/selection hooks, edit effects, mode, notifications
- [x] Add debug logging:
  - [x] "undo group pushed" with doc id + origin
  - [x] undo/redo pop/push and snapshot counts
  - [x] dispatched effects list

**Dependencies**: None
**Risk**: Low
**Rollback**: Revert commits; no design changes

---

## Phase 1: Extract UndoManager with Host Trait

**Purpose**: Isolate undo/redo logic into a component with clear boundaries.

### Tasks

- [x] Create `UndoManager` struct containing undo + redo stacks
- [x] Create `PreparedEdit` struct for pre-edit state
- [x] Implement `prepare_edit()` and `finalize_edit()` methods
- [x] Create `UndoHost` trait
- [x] Implement `UndoHost` for `Editor` by forwarding
- [x] Move `Editor::undo()` and `Editor::redo()` logic into `UndoManager`
- [x] Replace `Editor::undo()` body with delegation

### Code Sketch: UndoHost Trait

```rust
pub trait UndoHost {
    // Guardrails / policy
    fn guard_readonly(&mut self) -> bool;

    // Buffer/doc plumbing
    fn doc_id_for_buffer(&self, buffer_id: BufferId) -> DocumentId;

    // View snapshots
    fn collect_view_snapshots(&self, doc_id: DocumentId) -> HashMap<BufferId, ViewSnapshot>;
    fn capture_current_view_snapshots(&self, docs: &[DocumentId]) -> HashMap<BufferId, ViewSnapshot>;
    fn restore_view_snapshots(&mut self, snaps: &HashMap<BufferId, ViewSnapshot>);

    // Document-level history operations
    fn undo_documents(&mut self, docs: &[DocumentId]) -> bool;
    fn redo_documents(&mut self, docs: &[DocumentId]) -> bool;

    // Merge semantics for MergeWithCurrentGroup
    fn doc_insert_undo_active(&self, buffer_id: BufferId) -> bool;

    // Notifications
    fn notify_undo(&mut self);
    fn notify_redo(&mut self);
    fn notify_nothing_to_undo(&mut self);
    fn notify_nothing_to_redo(&mut self);
}
```

### Code Sketch: UndoManager

```rust
pub struct UndoManager {
    undo: Vec<EditorUndoGroup>,
    redo: Vec<EditorUndoGroup>,
}

pub struct PreparedEdit {
    pub affected_docs: SmallVec<[DocumentId; 1]>,
    pub pre_views: HashMap<BufferId, ViewSnapshot>,
    pub start_new_group: bool,
    pub origin: EditOrigin,
}

impl UndoManager {
    pub fn prepare_edit(
        &mut self,
        host: &dyn UndoHost,
        buffer_id: BufferId,
        undo: UndoPolicy,
        origin: EditOrigin,
    ) -> PreparedEdit {
        let doc_id = host.doc_id_for_buffer(buffer_id);
        let pre_views = host.collect_view_snapshots(doc_id);

        let start_new_group = match undo {
            UndoPolicy::MergeWithCurrentGroup => !host.doc_insert_undo_active(buffer_id),
            UndoPolicy::NoUndo => false,
            _ => true,
        };

        PreparedEdit { affected_docs: smallvec![doc_id], pre_views, start_new_group, origin }
    }

    pub fn finalize_edit(&mut self, applied: bool, prep: PreparedEdit) {
        if applied && prep.start_new_group {
            self.undo.push(EditorUndoGroup {
                affected_docs: prep.affected_docs.into_vec(),
                view_snapshots: prep.pre_views,
                origin: prep.origin,
            });
            self.redo.clear();
        }
    }

    pub fn undo(&mut self, host: &mut dyn UndoHost) -> bool {
        if !host.guard_readonly() { return false; }
        let Some(group) = self.undo.pop() else {
            host.notify_nothing_to_undo();
            return false;
        };

        let current = host.capture_current_view_snapshots(&group.affected_docs);
        let ok = host.undo_documents(&group.affected_docs);

        if ok {
            host.restore_view_snapshots(&group.view_snapshots);
            self.redo.push(EditorUndoGroup {
                affected_docs: group.affected_docs,
                view_snapshots: current,
                origin: group.origin,
            });
            host.notify_undo();
            true
        } else {
            self.undo.push(group);
            host.notify_nothing_to_undo();
            false
        }
    }

    // redo() follows same pattern
}
```

**Dependencies**: Phase 0
**Risk**: Medium (borrow checker + subtle snapshot restore ordering)
**Rollback**: Keep original `Editor::undo()` behind feature flag for one cycle

---

## Phase 2: Route Edit Push-Site Through UndoManager

**Purpose**: Centralize undo group creation logic.

### Tasks

- [x] Rewrite `apply_edit()` to use `prepare_edit()` / `finalize_edit()`
- [x] Verify redo stack clearing only occurs when group is pushed
- [x] Run behavior-lock tests from Phase 0

### Code Sketch: Converted apply_edit

```rust
pub(crate) fn apply_edit(
    &mut self,
    buffer_id: BufferId,
    tx: &Transaction,
    new_selection: Option<Selection>,
    undo: UndoPolicy,
    origin: EditOrigin,
) -> bool {
    // PREPARE (captures pre-edit snapshots and computes start_new_group)
    let prep = self.undo_manager.prepare_edit(self, buffer_id, undo, origin);

    // APPLY (unchanged)
    let applied = self.apply_transaction_inner(buffer_id, tx, new_selection, undo);

    // FINALIZE (pushes group and clears redo iff applied && start_new_group)
    self.undo_manager.finalize_edit(applied, prep);

    applied
}
```

**Dependencies**: Phase 1
**Risk**: Low-medium (group boundary semantics)
**Rollback**: Revert `apply_edit` to inline logic

---

## Phase 3: Introduce EditExecutor

**Purpose**: Create a single entry point for all edit operations.

### Tasks

- [x] Add `ApplyEditPolicy { undo, origin }` struct
- [x] Implement `EditExecutor<'a> { editor: &'a mut Editor }`
- [x] Add `apply_transaction()` wrapping UndoManager + inner apply
- [x] Add `execute_edit_op()` for EditOp handling
- [x] Add `paste()` for paste operations
- [x] Add `Editor::edit_executor()` method (EditorContext uses trait-based access)
- [x] Existing `apply_effects` uses `EditAccess` trait (correct abstraction for registry layer)

### Code Sketch: EditExecutor

```rust
pub struct EditExecutor<'a> {
    editor: &'a mut Editor,
}

impl<'a> EditExecutor<'a> {
    pub fn new(editor: &'a mut Editor) -> Self { Self { editor } }

    pub fn apply_transaction(
        &mut self,
        buffer_id: BufferId,
        tx: &Transaction,
        new_selection: Option<Selection>,
        policy: ApplyEditPolicy,
    ) -> bool {
        let prep = self.editor.undo_manager.prepare_edit(
            self.editor, buffer_id, policy.undo, policy.origin,
        );
        let applied = self.editor.apply_transaction_inner(
            buffer_id, tx, new_selection, policy.undo,
        );
        self.editor.undo_manager.finalize_edit(applied, prep);
        applied
    }

    pub fn execute_edit_op(&mut self, op: &EditOp, policy: ApplyEditPolicy) -> bool {
        let (buffer_id, tx, sel) = self.editor.compile_edit_op(op, policy);
        self.apply_transaction(buffer_id, &tx, sel, policy)
    }

    pub fn paste(&mut self, before: bool, policy: ApplyEditPolicy) -> bool {
        let (buffer_id, tx, sel) = self.editor.build_paste_tx(before);
        self.apply_transaction(buffer_id, &tx, sel, policy)
    }
}

#[derive(Clone, Copy)]
pub struct ApplyEditPolicy {
    pub undo: UndoPolicy,
    pub origin: EditOrigin,
}
```

**Dependencies**: Phase 2
**Risk**: Medium (borrow/lifetime issues, but mechanical)
**Rollback**: Keep old `ctx.edit()` path behind feature flag

### Implementation Notes

The original plan called for `EditorContext::edit_executor()` to return an `EditExecutor`.
This isn't possible due to crate dependencies:

- `EditorContext` lives in `xeno-registry` (the abstraction layer)
- `EditExecutor` lives in `xeno-editor` (the implementation)
- `xeno-registry` cannot depend on `xeno-editor` (would create circular dependency)

**Resolution**: The trait-based `EditAccess` interface remains the correct abstraction for
the registry layer. `EditExecutor` provides a convenient internal API within the editor
crate via `Editor::edit_executor()`. The `apply_effects` function continues to use
`ctx.edit()` which returns `Option<&mut dyn EditAccess>` - this is the right design
for cross-crate capability access.

### Adapter Pattern for Capability Traits

The current implementation uses "Option C": Editor implements EditAccess directly and
delegates to internal methods. This is a valid stepping stone toward Phase 7.

For Phase 7, the recommended pattern is "Option B" (EditorCore):

```rust
pub struct EditorCore {
    // buffers, documents, undo_manager, etc.
}

impl EditorCore {
    fn edit_executor(&mut self) -> EditExecutor<'_> { EditExecutor::new(self) }
}

impl EditAccess for EditorCore {
    fn execute_edit_op(&mut self, op: &EditOp) {
        self.edit_executor().execute_edit_op(op);
    }
    // ...
}
```

Then Editor becomes a façade holding `core` plus UI/layout/etc., and
`EditorContext::edit()` returns `Some(&mut self.core as &mut dyn EditAccess)`.

**Note**: `move_visual_vertical` is currently in `EditAccess` but is really a view/motion
operation, not an edit. Long-term, consider moving it to a separate `ViewAccess` or
`MotionAccess` trait. For now, keep it forwarding to a view subsystem rather than the
edit executor.

---

## Phase 4: Effect Nesting Refactor

**Purpose**: Organize Effect variants by domain for maintainability.

### Pre-Phase Checklist

Before starting, lock down effect ordering semantics:

- [x] Document invariants (e.g., SetSelection implies cursor hooks, EditOp runs after
      cursor/selection changes, Quit short-circuits or just sets outcome)
- [x] Add small tests that lock down ordering expectations
- [x] Consider adding `#[non_exhaustive]` to Effect enum if external crates match on it

### Tasks

- [x] Introduce nested enums: `ViewEffect`, `EditEffect`, `UiEffect`, `AppEffect`
- [x] Provide `From` conversions for backward compatibility
- [x] Update `ActionEffects` builder API to remain stable (this is the primary public surface)
- [x] Update `apply_effects` match to use nested structure
- [x] Add registry sanity test: `assert!(ACTIONS.len() >= N)` to catch linkme breakage

### Code Sketch: Nested Effects

```rust
pub enum Effect {
    View(ViewEffect),
    Edit(EditEffect),
    Ui(UiEffect),
    App(AppEffect),
}

pub enum ViewEffect {
    SetCursor(CharIdx),
    SetSelection(Selection),
    ScreenMotion { position: ScreenPosition, count: usize },
    Scroll { direction: Direction, amount: ScrollAmount, extend: bool },
    VisualMove { direction: Direction, count: usize, extend: bool },
    Search { direction: SeqDirection, add_selection: bool },
    UseSelectionAsSearch,
}

pub enum EditEffect {
    EditOp(EditOp),
    Paste { before: bool },
}

pub enum UiEffect {
    Notify(Notification),
    Error(String),
    OpenPalette,
    ClosePalette,
    ExecutePalette,
    ForceRedraw,
}

pub enum AppEffect {
    SetMode(Mode),
    Pending(PendingAction),
    FocusBuffer(SeqDirection),
    FocusSplit(SpatialDirection),
    Split(Axis),
    CloseSplit,
    CloseOtherBuffers,
    Quit { force: bool },
    QueueCommand { name: &'static str, args: Vec<String> },
}
```

**Dependencies**: Phase 3
**Risk**: Low-medium (widespread but mechanical edits)
**Rollback**: Keep flat enum behind `#[cfg(feature="flat_effects")]`

### Implementation Notes

**Keep builders as the stable API**: The `ActionEffects::{set_cursor, set_selection,
edit_op, notify, ...}` builder methods should remain the primary public surface.
Treat the enum layout as an internal implementation detail. This makes Phase 4
mostly "update interpreter + update internal enum shape" rather than "update 50
call sites."

**Avoid "editor-y" effects**: Keep effects high-level (EditOp, Paste, QueueCommand)
rather than low-level ("apply this Transaction" or "run this EditPlan"). Low-level
effects would force editor types into the registry crate, recreating the circular
dependency problem from Phase 3.

**Registry churn prevention**: When changing Effect or related types, follow this order:
1. Add new fields/types
2. Wire builders/adapters
3. Migrate call sites
4. Remove legacy fields

This keeps breakage local and matches the phase discipline.

---

## Phase 5: RegistryMeta Normalization

**Purpose**: Reduce boilerplate across all registry types.

### Tasks

- [x] Introduce `RegistryMeta` in registry core crate
- [x] For each main extensible `*Def`: add `meta: RegistryMeta` field
  - [x] `ActionDef` uses `meta: RegistryMeta`
  - [x] `MotionDef` uses `meta: RegistryMeta`
  - [x] `CommandDef` uses `meta: RegistryMeta`
  - [x] `TextObjectDef` uses `meta: RegistryMeta`
  - Note: Specialized types (GutterDef, HookDef, etc.) don't have all RegistryMeta
    fields and continue using impl_registry_metadata!
- [x] Update macros to build `RegistryMeta`:
  - [x] `action!` produces `ActionDef { meta, short_desc, handler }`
  - [x] `motion!` produces `MotionDef { meta, handler }`
  - [x] `command!` produces `CommandDef { meta, handler, user_data }`
  - [x] `text_object!` produces `TextObjectDef { meta, trigger, alt_triggers, inner, around }`
- [x] Add `RegistryEntry` trait for introspection
- [x] Add introspection helpers:
  - [x] `list_actions()` sorted by priority/name (via all_actions())
  - [x] Collision reporting for duplicate ids/aliases (via registry_diag commands)
  - [x] Generic help renderers (via :help command)

### Code Sketch: RegistryMeta

```rust
#[derive(Clone, Copy)]
pub struct RegistryMeta {
    pub id: &'static str,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub priority: i16,
    pub source: RegistrySource,
    pub required_caps: &'static [Capability],
    pub flags: u32,
}

pub trait RegistryEntry {
    fn meta(&self) -> &RegistryMeta;

    fn id(&self) -> &'static str { self.meta().id }
    fn name(&self) -> &'static str { self.meta().name }
    fn aliases(&self) -> &'static [&'static str] { self.meta().aliases }
    fn description(&self) -> &'static str { self.meta().description }
    fn priority(&self) -> i16 { self.meta().priority }
    fn source(&self) -> RegistrySource { self.meta().source }
    fn required_caps(&self) -> &'static [Capability] { self.meta().required_caps }
    fn flags(&self) -> u32 { self.meta().flags }
}

// Example updated ActionDef
pub struct ActionDef {
    pub meta: RegistryMeta,
    pub short_desc: &'static str,
    pub handler: fn(&ActionContext) -> ActionResult,
}

impl RegistryEntry for ActionDef {
    fn meta(&self) -> &RegistryMeta { &self.meta }
}
```

**Dependencies**: Phase 4 (optional; can run in parallel)
**Risk**: Medium (macro churn + distributed compilation errors)
**Rollback**: Do registry-by-registry, keep old layout behind feature flags

---

## Phase 6: Capability Gating Consolidation

**Purpose**: Ensure all user-invoked operations route through a single gate.

### Tasks

- [x] Create `Editor::run_invocation(Invocation)` that always checks capabilities
- [x] Ensure all entry points route through it:
  - [x] Keymap action dispatch (uses existing `execute_action` with same capability checking)
  - [x] Command palette (queues commands, handled by drain_command_queue)
  - [x] Ex commands / queued commands (routes through `run_invocation`)
  - [x] Hook-triggered invocations (hooks emit after capability checking)
- [x] Add `InvocationPolicy { enforce_caps: bool, enforce_readonly: bool }`
- [x] Start with "log-only mode" before enforcing

**Dependencies**: Phase 3 (edit pipeline centralized)
**Risk**: Medium-high (behavior changes if bypass existed)
**Rollback**: Log-only mode first, then flip to enforcing

### Implementation Notes

**Choke point must live in the editor crate, not registry**: Because the registry
layer only declares capabilities (via traits like `EditorCapabilities`), the actual
enforcement must happen in editor entrypoints. Don't try to centralize gating inside
registry types or EditorContext.

The single `run_invocation(...)` function should:
1. Resolve the def (action/command)
2. Check caps using `EditorContext::check_all_capabilities`
3. Enforce readonly / policy
4. Emit hooks
5. Execute handler
6. Interpret effects

Registry stays declarative; editor becomes the policy engine.

### Implementation Details

**New types in `crates/editor/src/types/invocation.rs`:**

```rust
pub enum Invocation {
    Action { name, count, extend, register },
    ActionWithChar { name, count, extend, register, char_arg },
    Command { name, args },
    EditorCommand { name, args },
}

pub struct InvocationPolicy {
    pub enforce_caps: bool,
    pub enforce_readonly: bool,
}

pub enum InvocationResult {
    Ok,
    Quit,
    ForceQuit,
    NotFound(String),
    CapabilityDenied(Capability),
    ReadonlyDenied,
    CommandError(String),
}
```

**Entry point consolidation:**

- `drain_command_queue` now routes all commands through `run_invocation`
- `execute_action` / `execute_action_with_char` retain existing capability checking
  (same pattern as `run_action_invocation`) for sync keymap dispatch
- Commands now get capability checking via `required_caps()` from RegistryMeta

**Log-only mode:** Current deployment uses `InvocationPolicy::log_only()` which
logs capability violations without blocking. Flip to `InvocationPolicy::enforcing()`
when ready for production enforcement.

---

## Phase 7: Optional - Reduce Editor God-Object Pressure

**Purpose**: Structural cleanup for long-term maintainability.

### Tasks

- [ ] Split `Editor` into `EditorCore` + components:
  - [ ] `UndoManager` (already done in Phase 1)
  - [ ] `HookEngine`
  - [ ] `Workspace` / `WindowManager`
  - [ ] `OverlayManager`
- [ ] `Editor` becomes a thin facade delegating to components
- [ ] Move capability trait impls (`EditAccess`, etc.) to `EditorCore`

**Dependencies**: Phases 1-3 strongly recommended
**Risk**: High (structural churn)
**Rollback**: Series of "extract struct" commits, revert most recent if broken

### Implementation Notes

**The constraint from Phase 3 helps here**: Traits remain stable while implementation
evolves. When you split Editor into components, you keep implementing the same registry
traits (`EditorCapabilities`, `EditAccess`, `UndoAccess`, etc.) on a façade struct.
Internally, those methods delegate to `UndoManager`, `EditExecutor`, etc.

**Recommended structure**:

```rust
pub struct Editor {
    core: EditorCore,  // buffers, documents, undo_manager
    ui: UiManager,
    layout: LayoutManager,
    // ...
}

impl EditAccess for EditorCore {
    fn execute_edit_op(&mut self, op: &EditOp) {
        self.edit_executor().execute_edit_op(op);
    }
    // ...
}
```

**Borrow conflict prevention**: If your EditAccess implementation borrows `&mut Editor`,
component extraction can introduce borrow conflicts. Fix by making Editor own a core
struct and have capability adapters borrow only the minimal parts they need
(e.g., `&mut EditorCore + &mut UndoManager`), or use "facade methods"
(`Editor::apply_edit(...)`) that encapsulate borrowing.

---

## Cross-Phase Dependencies

```
Phase 0 (Guardrails)
    │
    v
Phase 1 (UndoManager) ─────────────────┐
    │                                  │
    v                                  │
Phase 2 (apply_edit conversion)        │
    │                                  │
    v                                  │
Phase 3 (EditExecutor)                 │
    │                                  │
    ├──────────────┐                   │
    v              v                   v
Phase 4        Phase 6            Phase 5
(Effect        (Capability        (RegistryMeta)
 nesting)       gating)            [parallel]
    │
    v
Phase 7 (Editor split - optional)
```

---

## Distributed Slice Registry Gotchas

Watch for these issues during refactoring:

1. **Duplicate IDs/aliases** - Add startup collision check per registry
2. **Feature flags silently remove registrations** - CI feature-matrix builds essential
3. **Tests are separate binaries** - Registry contents can differ between test/app
4. **Incremental linking surprises** - Stick to established linkme patterns
5. **Cross-crate version skew** - Keep registry crate versions unified

### Recommended Safety Net

```rust
#[test]
fn registry_sanity_check() {
    assert!(ACTIONS.len() >= 50, "Expected at least 50 actions registered");
    assert!(MOTIONS.len() >= 20, "Expected at least 20 motions registered");
    // ... etc
}
```

---

## Code Scale Management (24+ Crates)

### Critical Practices

1. **Enforce dependency direction mechanically**
   - Add layering rules document
   - CI check to prevent UI crates creeping into core

2. **Feature-flag hygiene in CI**
   - Build/test: `--no-default-features`
   - Build/test: `--all-features`
   - Build/test: realistic shipped feature sets

3. **Avoid workspace public API sprawl**
   - Default to `pub(crate)`
   - Re-export intentionally from facade crates

4. **Build-time scalability**
   - Split heavy proc-macro crates
   - Avoid cascading syn/quote dependencies
   - Use `cargo nextest` for fast test cycles
   - Consider `cargo hakari` if feature unification painful

5. **Observability for extension points**
   - Add "dump registries" commands early
   - Debug weird dispatch and missing registrations

---

## General Rollback Strategy

1. **Keep old implementations accessible** for one cycle (feature flag or private module):
   - Undo/redo path
   - Flat effects enum
   - Legacy registry struct layout

2. **Make each phase land as small revertable commits**:
   - "Introduce new type"
   - "Wire delegation"
   - "Switch call sites"
   - "Delete old path"

3. **Prefer adapter layers over big bangs**:
   - Example: `Effect::SetCursor` constructor keeps working while internally producing nested variants

---

## Summary Checklist

### Phase 0: Guardrails
- [x] Behavior-lock tests for undo/redo
- [x] Behavior-lock tests for effects
- [x] Debug logging infrastructure

### Phase 1: UndoManager
- [x] UndoManager struct
- [x] PreparedEdit struct
- [x] UndoHost trait
- [x] Editor impl for UndoHost
- [x] Delegation from Editor

### Phase 2: apply_edit
- [x] Convert to prepare/finalize pattern
- [x] Verify behavior preservation

### Phase 3: EditExecutor
- [x] ApplyEditPolicy struct
- [x] EditExecutor struct
- [x] Editor::edit_executor() method
- [x] apply_effects uses trait-based access

### Phase 4: Effect Nesting
- [x] Pre-phase: Document effect ordering invariants
- [x] Pre-phase: Add ordering lock-down tests
- [x] Pre-phase: Consider `#[non_exhaustive]` on Effect
- [x] Nested enum structure
- [x] From conversions
- [x] Builder API preservation
- [x] Interpreter update
- [x] Registry sanity test (action count >= N)

### Phase 5: RegistryMeta
- [x] RegistryMeta struct
- [x] RegistryEntry trait
- [x] Update *Def structs (ActionDef, MotionDef, CommandDef, TextObjectDef)
- [x] Update macros (action!, motion!, command!, text_object!)
- [x] Introspection helpers (collision detection via registry_diag)

Note: Specialized types (GutterDef, HookDef, StatuslineSegmentDef, etc.) were reviewed and
kept using impl_registry_metadata! since they don't have all RegistryMeta fields (no aliases,
required_caps, flags). The main extensible registry types have been migrated.

### Phase 6: Capability Gating
- [x] Unified entry point (`Editor::run_invocation`)
- [x] Route all paths through it (commands via run_invocation, actions via execute_action)
- [x] InvocationPolicy (enforce_caps, enforce_readonly)
- [x] Log-only mode first (InvocationPolicy::log_only())
- [x] Note: Choke point in editor, not registry

### Phase 7: Editor Split (Optional)
- [ ] Extract HookEngine
- [ ] Extract Workspace
- [ ] Extract OverlayManager
- [ ] Facade pattern (Editor delegates to EditorCore)
- [ ] Move capability trait impls to EditorCore

---

*Generated from collaborative analysis with Claude and ChatGPT.*
