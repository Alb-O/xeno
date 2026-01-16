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

- [ ] Add `ApplyEditPolicy { undo, origin }` struct
- [ ] Implement `EditExecutor<'a> { editor: &'a mut Editor }`
- [ ] Add `apply_transaction()` wrapping UndoManager + inner apply
- [ ] Add `execute_edit_op()` for EditOp handling
- [ ] Add `paste()` for paste operations
- [ ] Update `EditorContext::edit_executor()` to return by value
- [ ] Update `apply_effects` to use `edit_executor()`

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

---

## Phase 4: Effect Nesting Refactor

**Purpose**: Organize Effect variants by domain for maintainability.

### Tasks

- [ ] Introduce nested enums: `ViewEffect`, `EditEffect`, `UiEffect`, `AppEffect`
- [ ] Provide `From` conversions for backward compatibility
- [ ] Update `ActionEffects` builder API to remain stable
- [ ] Update `apply_effects` match to use nested structure

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

---

## Phase 5: RegistryMeta Normalization

**Purpose**: Reduce boilerplate across all registry types.

### Tasks

- [ ] Introduce `RegistryMeta` in registry core crate
- [ ] For each `*Def`: add `meta: RegistryMeta` field
- [ ] Update macros to build `RegistryMeta`:
  - [ ] `action!` produces `ActionDef { meta, short_desc, handler }`
  - [ ] `motion!` produces `MotionDef { meta, handler }`
  - [ ] `gutter!` produces `GutterDef { meta, default_enabled, width, render }`
  - [ ] (continue for other registries)
- [ ] Add `RegistryEntry` trait for introspection
- [ ] Add introspection helpers:
  - [ ] `list_actions()` sorted by priority/name
  - [ ] Collision reporting for duplicate ids/aliases
  - [ ] Generic help renderers

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

- [ ] Create `Editor::run_entrypoint(Invocation)` that always checks capabilities
- [ ] Ensure all entry points route through it:
  - [ ] Keymap action dispatch
  - [ ] Command palette
  - [ ] Ex commands / queued commands
  - [ ] Hook-triggered invocations (decide bypass policy)
- [ ] Add `InvocationPolicy { enforce_caps: bool, enforce_readonly: bool }`
- [ ] Start with "log-only mode" before enforcing

**Dependencies**: Phase 3 (edit pipeline centralized)
**Risk**: Medium-high (behavior changes if bypass existed)
**Rollback**: Log-only mode first, then flip to enforcing

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

**Dependencies**: Phases 1-3 strongly recommended
**Risk**: High (structural churn)
**Rollback**: Series of "extract struct" commits, revert most recent if broken

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
- [ ] ApplyEditPolicy struct
- [ ] EditExecutor struct
- [ ] Update EditorContext
- [ ] Update apply_effects

### Phase 4: Effect Nesting
- [ ] Nested enum structure
- [ ] From conversions
- [ ] Builder API preservation
- [ ] Interpreter update

### Phase 5: RegistryMeta
- [ ] RegistryMeta struct
- [ ] RegistryEntry trait
- [ ] Update all *Def structs
- [ ] Update all macros
- [ ] Introspection helpers

### Phase 6: Capability Gating
- [ ] Unified entry point
- [ ] Route all paths through it
- [ ] InvocationPolicy
- [ ] Log-only mode first

### Phase 7: Editor Split (Optional)
- [ ] Extract HookEngine
- [ ] Extract Workspace
- [ ] Extract OverlayManager
- [ ] Facade pattern

---

*Generated from collaborative analysis with Claude and ChatGPT.*
