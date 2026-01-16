# Xeno Refactoring Roadmap: Edit Gate, Undo, and Boundaries

## Goals

1. **Single, authoritative edit gate** - undo/redo, readonly, modified/version, syntax scheduling happen in one place
2. **Clean separation** of document history vs editor/view history
3. **Tight invariants** - no direct mutable access to Document fields or RwLock guards escaping
4. **Typed errors and policies** - `Result<_, EditError>`, `UndoPolicy`, `SyntaxPolicy`
5. **Migration without a flag day**

---

## Phase 1: Stabilize Invariants and Introduce Typed Policies

### Action Items

- [x] Make Document core fields private: `content`, `modified`, `readonly`, undo/redo stacks, syntax state, `version`
- [x] Add read-only getters (`content()`, `is_modified()`, `is_readonly()`, `version()`, etc.)
- [x] Introduce `EditError` and convert readonly checks to `Result<(), EditError>` at key edit entrypoints
- [x] Introduce `UndoPolicy` and `SyntaxPolicy` enums (used even before the new commit gate exists)
- [x] Add a `CommitResult` shape and return it from existing "apply transaction" paths as a stub

### Key New Abstractions

```rust
// crates/editor/src/edit/types.rs (or primitives if shared)

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("document is read-only: {scope:?} ({reason})")]
    ReadOnly { scope: ReadOnlyScope, reason: ReadOnlyReason },

    #[error("invalid selection: {0}")]
    InvalidSelection(String),

    #[error("transaction apply failed: {0}")]
    ApplyFailed(String),

    #[error("undo/redo unavailable: {0}")]
    History(String),

    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Copy)]
pub enum ReadOnlyScope {
    Buffer,
    Document,
}

#[derive(Debug, Clone)]
pub enum ReadOnlyReason {
    FlaggedReadOnly,
    PermissionDenied,
    BufferOverride,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub enum UndoPolicy {
    /// No undo record (rare; e.g., ephemeral or preview edits).
    NoUndo,
    /// Normal: this commit becomes an undo step.
    Record,
    /// Merge into current group (e.g., insert-typing run).
    MergeWithCurrentGroup,
    /// Explicit boundary: end current group and start a new one.
    Boundary,
}

#[derive(Debug, Clone, Copy)]
pub enum SyntaxPolicy {
    /// Do not touch syntax (rare; internal ops).
    None,
    /// Mark dirty; do work lazily (e.g., next render).
    MarkDirty,
    /// Apply incremental update if available; else mark dirty.
    IncrementalOrDirty,
    /// Force immediate full reparse (temporary compatibility mode).
    FullReparseNow,
}

#[derive(Debug, Clone)]
pub struct CommitResult {
    pub applied: bool,
    pub version_before: u64,
    pub version_after: u64,
    pub selection_after: Option<Selection>,
    pub syntax_changed: bool,
    pub undo_recorded: bool,
}
```

### Migration Notes

- Introduce these types without changing behavior yet: wrap old boolean guards and return stub `CommitResult`
- Keep existing snapshot undo intact for now

---

## Phase 2: Stop Leaking RwLock Guards (Closure-Based Lock Pattern)

### Action Items

- [x] Deprecate public `doc()` / `doc_mut()` returning guards
- [x] Add `Buffer::with_doc` / `Buffer::with_doc_mut` closure APIs
- [x] Update all call sites to use closure APIs (mechanical refactor)
- [ ] Add a lint-like rule in review: no lock guards escape the module

### Closure-Based Lock Pattern

```rust
// crates/editor/src/buffer/mod.rs
impl Buffer {
    pub fn with_doc<R>(&self, f: impl FnOnce(&Document) -> R) -> R {
        let guard = self.document.read().expect("doc lock poisoned");
        f(&*guard)
    }

    pub fn with_doc_mut<R>(&self, f: impl FnOnce(&mut Document) -> R) -> R {
        let mut guard = self.document.write().expect("doc lock poisoned");
        f(&mut *guard)
    }
}
```

### Migration Notes

- This is the highest ROI "safety upgrade" with minimal design risk
- After this, you can enforce that edits and syntax/undo operations occur under deliberate lock scope

---

## Phase 3: Introduce the Document Edit Gate (`Document::commit`)

### Action Items

- [x] Add `EditCommit` type
- [x] Implement `Document::commit(EditCommit) -> Result<CommitResult, EditError>`:
  - [x] readonly check
  - [x] undo recording (still snapshot-based initially)
  - [x] apply transaction to content
  - [x] bump version / mark modified
  - [x] clear redo
  - [x] syntax policy handling (initially `FullReparseNow` to match existing behavior)
- [x] Replace all direct `Transaction.apply(doc.content)` and "caller saves undo" flows with `doc.commit(...)`
- [x] Remove or hide old `apply_transaction*` APIs (or make them delegate to commit)

### Key New Abstractions

```rust
#[derive(Debug, Clone)]
pub struct EditCommit {
    pub tx: Transaction,
    pub undo: UndoPolicy,
    pub syntax: SyntaxPolicy,
    pub origin: EditOrigin,
    /// Optional selection override produced by the edit planner.
    pub selection_after: Option<Selection>,
}

/// Useful for grouping, telemetry, debugging.
#[derive(Debug, Clone)]
pub enum EditOrigin {
    EditOp { id: &'static str },
    Command { name: String },
    MacroReplay,
    Lsp,
    Internal(&'static str),
}
```

```rust
// crates/editor/src/buffer/document.rs
impl Document {
    pub fn commit(&mut self, commit: EditCommit) -> Result<CommitResult, EditError> {
        let version_before = self.version;

        self.ensure_writable()?; // -> Result<(), EditError>

        // Undo step (snapshot backend for now):
        let mut undo_recorded = false;
        match commit.undo {
            UndoPolicy::NoUndo => {}
            _ => {
                self.push_undo_snapshot(); // existing behavior
                undo_recorded = true;
            }
        }

        // Apply transaction:
        commit.tx.apply(&mut self.content);
        self.modified = true;
        self.version = self.version.wrapping_add(1);
        self.redo_stack.clear();

        // Selection is stored on buffer/view layer later, but commit can return it:
        let selection_after = commit.selection_after.or(commit.tx.selection.clone());

        // Syntax policy (compatibility mode initially):
        let syntax_changed = match commit.syntax {
            SyntaxPolicy::None => false,
            SyntaxPolicy::FullReparseNow => {
                self.reparse_syntax_full();
                true
            }
            SyntaxPolicy::MarkDirty => {
                self.syntax_dirty = true;
                false
            }
            SyntaxPolicy::IncrementalOrDirty => {
                self.syntax_dirty = true;
                false
            }
        };

        Ok(CommitResult {
            applied: true,
            version_before,
            version_after: self.version,
            selection_after,
            syntax_changed,
            undo_recorded,
        })
    }

    fn ensure_writable(&self) -> Result<(), EditError> {
        if self.readonly {
            return Err(EditError::ReadOnly {
                scope: ReadOnlyScope::Document,
                reason: ReadOnlyReason::FlaggedReadOnly,
            });
        }
        Ok(())
    }
}
```

### Migration Notes

- Behavior-preserving: `commit.undo != NoUndo` can simply call `push_undo_snapshot()` exactly like today
- Keep `reparse_syntax` behavior as-is first (`FullReparseNow`), then improve later
- This phase kills the "caller responsible for save_undo_state + syntax update" problem immediately

---

## Phase 4: Split Document History vs Editor/View History

### Action Items

- [x] Redefine document history entries to be document-only (no `BufferId`)
- [x] Move selection/cursor restoration into editor-level undo groups
- [x] Create a `ViewSnapshot` (selection/cursor + other view state you care about)
- [x] Update editor undo/redo to:
  - [x] capture view snapshots at group boundaries
  - [x] call `doc.undo()`/`doc.redo()` once per affected document
  - [x] restore view snapshots for affected buffers

### Key Separation Types

```rust
/// Document-level undo step: document state only.
pub enum DocumentUndoStep {
    /// Temporary backend (current behavior):
    Snapshot { rope: Rope, version: u64 },

    /// Future backend:
    Transaction { undo: Transaction, redo: Transaction },
}

/// Editor-level group: view state + which documents to undo.
pub struct EditorUndoGroup {
    pub affected_docs: Vec<DocId>,
    pub view_snapshots: std::collections::HashMap<BufferId, ViewSnapshot>,
    pub origin: EditOrigin,
}

#[derive(Debug, Clone)]
pub struct ViewSnapshot {
    pub cursor: CharIdx,
    pub selection: Selection,
    pub scroll_line: usize,
    pub scroll_segment: usize,
    pub goal_column: Option<usize>,
    // add more if needed
}
```

### Migration Notes

- First, keep document undo as snapshot-based but remove selection maps from document history
- Editor already groups buffers; adapt it to store `ViewSnapshot` and restore it
- Do not attempt selection mapping through changesets yet; restore exact view snapshots

---

## Phase 5: Introduce UndoStore Backend and Switch to Transaction-Based Undo

### Action Items

- [ ] Introduce `UndoStore` trait (document-owned)
- [ ] Implement `SnapshotUndoStore` using current snapshot behavior
- [ ] Implement `TxnUndoStore` using `Transaction + invert()`:
  - [ ] store `(undo_tx, redo_tx)` per step
  - [ ] apply on undo/redo via `Document::commit` with `UndoPolicy::NoUndo` to avoid recursion
- [ ] Move `MAX_UNDO` enforcement into the undo store
- [ ] Replace document's `undo_stack: Vec<HistoryEntry>` with `undo: Box<dyn UndoStore>` (or enum if you want no vtables)

### UndoStore Trait

```rust
pub trait UndoStore {
    fn clear_redo(&mut self);
    fn record_commit(
        &mut self,
        before: &DocumentSnapshot,
        commit: &EditCommit,
        after: &DocumentSnapshot,
    ) -> Result<(), EditError>;

    fn can_undo(&self) -> bool;
    fn can_redo(&self) -> bool;

    fn undo(&mut self, doc: &mut Document) -> Result<(), EditError>;
    fn redo(&mut self, doc: &mut Document) -> Result<(), EditError>;
}

pub struct DocumentSnapshot {
    pub rope: Rope,
    pub version: u64,
    // optional: syntax versioning, etc.
}
```

A transaction-based step:

```rust
pub struct TxnUndoStep {
    pub undo: Transaction,
    pub redo: Transaction,
    pub version_before: u64,
    pub version_after: u64,
}
```

### Migration Notes

- Start by wiring `SnapshotUndoStore` behind the trait and ensure zero behavior change
- Then implement `TxnUndoStore` and add a feature flag or config switch to opt-in
- Only after transaction undo is stable should you delete snapshot code

---

## Phase 6: Make Syntax Updates Policy-Driven and Cheaper

### Action Items

- [ ] Change default commit syntax policy from `FullReparseNow` -> `MarkDirty`
- [ ] Ensure render path or a background job triggers reparse when dirty
- [ ] Add incremental updates (optional / later):
  - [ ] use `ChangeSet` ranges to feed incremental parser
  - [ ] fall back to full reparse if incremental fails

### Migration Notes

- Do `MarkDirty` + lazy reparse first; it preserves correctness with big perf wins
- Incremental parsing is optional and can be tackled after undo refactor is complete

---

## Phase 7: Tighten EditOp Execution - Compile -> Commit

### Action Items

- [ ] Add `EditOp::compile(&Context) -> Result<EditPlan, EditError>`:
  - [ ] validate conflicts (multiple boundaries, contradictory cursor moves)
  - [ ] normalize order (pre -> selection -> transform -> post)
  - [ ] resolve policies (`UndoPolicy`/`SyntaxPolicy`) in one place
- [ ] Make executor:
  - [ ] compute plan (no mutation)
  - [ ] call `doc.commit(commit)` once
  - [ ] apply view changes based on `CommitResult` + post-effects
- [ ] Delete "sprinkled save_undo_state calls" in transforms entirely

### Migration Notes

- You can keep current phased executor but route actual edits through commit
- Once stable, make compile stage pure for easy testing

---

## Incremental Adoption Map

This is the recommended step-by-step order that maintains a running editor:

1. **Types first**: add `EditError`, `UndoPolicy`, `SyntaxPolicy`, `CommitResult` with stub usage
2. **Lock pattern**: replace all guard leaks with `with_doc`/`with_doc_mut` (mechanical)
3. **Edit gate**: implement `Document::commit` but internally keep snapshot undo + full reparse to match behavior
4. **Route edits**: move all edit paths to call commit (delete direct rope mutations)
5. **History split**: remove buffer selections from document undo; add editor-level `ViewSnapshot` groups
6. **Undo backend abstraction**: introduce `UndoStore`, start with snapshot implementation
7. **Transaction undo**: implement `TxnUndoStore`, test heavily, then flip default
8. **Syntax policy**: shift default to `MarkDirty` + lazy parse; later incremental
9. **Compile stage**: make `EditOp` compile into an `EditCommit` + effects

At each step, the editor should still build and function with minimal behavioral drift.

---

## Testing Strategy

### 1) Property Tests (High Value)

Use `proptest` (or `quickcheck`) on Transaction/undo invariants:

- [ ] **Undo round-trip**: for random documents and random transactions, `apply tx`, then `apply tx.invert()` => document content returns to original
- [ ] **Redo round-trip**: `apply tx`, undo, redo => content equals post-apply
- [ ] **Selection mapping sanity** (if using mapped selections):
  - selection stays within bounds after apply/invert
  - mapping is stable across repeated operations
- [ ] **Commit gate invariants**:
  - if readonly, commit returns `EditError::ReadOnly` and content unchanged
  - commit increments version exactly once per applied edit
  - redo stack clears on commit (when undo recorded)

If you keep snapshots temporarily, still property-test that undo/redo restores exact rope content.

### 2) Unit Tests for New Abstractions

- [ ] `Document::commit`: sets modified, increments version, clears redo, records undo based on policy
- [ ] `UndoStore` implementations:
  - snapshot store respects `MAX_UNDO`
  - txn store correctly records `(undo, redo)` pairs and replays them
- [ ] `EditorUndoGroup`: multi-buffer selection restoration works even if buffers are destroyed/created (ensure robust handling)
- [ ] Lock closure APIs: ensure no deadlocks in common nested patterns (at least test that closures are short and don't re-enter in ways you forbid)

### 3) Integration Tests (Editor-Level)

- [ ] Scripted keystroke tests for:
  - insert typing grouping (merge behavior)
  - delete/change/yank behaviors
  - multi-split document edits + undo/redo
  - redo invalidation after new edit
- [ ] "Long edit" tests ensuring no quadratic behavior (basic perf assertions / timeouts)

### 4) Regression Harness

- [ ] Add a "replay log" format (optional): record a sequence of EditOps/commands and assert final buffer content + cursor/selection
- [ ] Use it to lock in behavior during refactors

---

## Appendix: Summary of Key Types

```rust
pub struct EditCommit {
    pub tx: Transaction,
    pub undo: UndoPolicy,
    pub syntax: SyntaxPolicy,
    pub origin: EditOrigin,
    pub selection_after: Option<Selection>,
}

pub struct CommitResult {
    pub applied: bool,
    pub version_before: u64,
    pub version_after: u64,
    pub selection_after: Option<Selection>,
    pub syntax_changed: bool,
    pub undo_recorded: bool,
}

pub enum EditError {
    ReadOnly { scope: ReadOnlyScope, reason: ReadOnlyReason },
    InvalidSelection(String),
    ApplyFailed(String),
    History(String),
    Internal(String),
}

pub trait UndoStore {
    fn record_commit(&mut self, before: &DocumentSnapshot, commit: &EditCommit, after: &DocumentSnapshot) -> Result<(), EditError>;
    fn undo(&mut self, doc: &mut Document) -> Result<(), EditError>;
    fn redo(&mut self, doc: &mut Document) -> Result<(), EditError>;
    fn can_undo(&self) -> bool;
    fn can_redo(&self) -> bool;
    fn clear_redo(&mut self);
}

pub enum DocumentUndoStep {
    Snapshot { rope: Rope, version: u64 },
    Transaction { undo: Transaction, redo: Transaction },
}

pub struct EditorUndoGroup {
    pub affected_docs: Vec<DocId>,
    pub view_snapshots: HashMap<BufferId, ViewSnapshot>,
    pub origin: EditOrigin,
}

pub enum UndoPolicy { NoUndo, Record, MergeWithCurrentGroup, Boundary }
pub enum SyntaxPolicy { None, MarkDirty, IncrementalOrDirty, FullReparseNow }

impl Buffer {
    pub fn with_doc<R>(&self, f: impl FnOnce(&Document) -> R) -> R;
    pub fn with_doc_mut<R>(&self, f: impl FnOnce(&mut Document) -> R) -> R;
}
```

---

## Key Findings That Drove This Roadmap

### A) Document history storing per-buffer selections is the wrong layer

`HistoryEntry { doc: Rope, selections: HashMap<BufferId, Selection> }` makes the document responsible for view state. This becomes brittle when:
- buffers are created/destroyed (history retains dead BufferIds)
- multiple buffers share one doc but diverge in selection/cursor semantics
- future features add per-view state beyond selection (scroll, wrap, mode-local state)

**Rule of thumb**: Document undo should store document state only. Buffer/view undo belongs at the editor level.

### B) Snapshot-based undo + full syntax reparse is a performance trap

Even if Rope clones are cheap structurally, keeping snapshots alive pins old rope nodes and grows memory under real editing. Then `reparse_syntax` on every undo/redo makes undo feel "heavy".

You already have `Transaction + ChangeSet + invert() + selection mapping` primitives. That's screaming for a transaction-based undo log.

### C) Locking protocol is "whoever grabs the guard wins"

`Arc<RwLock<Document>>` + public `doc_mut()` returning a WriteGuard with no protocol means:
- edits can happen while someone else expects stability
- syntax updates might run under a write lock accidentally (long lock hold)
- multi-document operations can deadlock if two locks are taken in different orders

Even if you're single-threaded today, this shape makes it hard to become more concurrent later.

### D) Public fields on Document are an invariant leak

If `content`, `modified`, `readonly` are directly mutable, then "modified" and versioning will eventually desync from undo, syntax state, file state, etc.
