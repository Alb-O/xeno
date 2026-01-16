# Xeno Refactoring Roadmap: Edit Gate, Undo, and Boundaries

## Goals

1. **Single, authoritative edit gate** - undo/redo, readonly, modified/version, syntax scheduling happen in one place
2. **Clean separation** of document history vs editor/view history
3. **Tight invariants** - no direct mutable access to Document fields or RwLock guards escaping
4. **Typed errors and policies** - `Result<_, EditError>`, `UndoPolicy`, `SyntaxPolicy`
5. **Migration without a flag day**

______________________________________________________________________

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

______________________________________________________________________

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

______________________________________________________________________

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

______________________________________________________________________

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

______________________________________________________________________

## Phase 5: Introduce UndoStore Backend and Switch to Transaction-Based Undo

### Action Items

- [x] Introduce `UndoStore` trait (document-owned)
- [x] Implement `SnapshotUndoStore` using current snapshot behavior
- [x] Implement `TxnUndoStore` using `Transaction + invert()`:
  - [x] store `(undo_tx, redo_tx)` per step
  - [x] apply on undo/redo via `Document::commit` with `UndoPolicy::NoUndo` to avoid recursion
- [x] Move `MAX_UNDO` enforcement into the undo store
- [x] Replace document's `undo_stack: Vec<HistoryEntry>` with `undo: Box<dyn UndoStore>` (or enum if you want no vtables)

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

______________________________________________________________________

## Phase 6: Make Syntax Updates Policy-Driven and Cheaper

### Action Items

- [x] Change default commit syntax policy from `FullReparseNow` -> `MarkDirty`
- [x] Ensure render path or a background job triggers reparse when dirty
- [x] Add incremental updates (optional / later):
  - [x] use `ChangeSet` ranges to feed incremental parser
  - [x] fall back to full reparse if incremental fails

### Migration Notes

- Do `MarkDirty` + lazy reparse first; it preserves correctness with big perf wins
- Incremental parsing is optional and can be tackled after undo refactor is complete

______________________________________________________________________

## Phase 7: Tighten EditOp Execution - Compile -> Commit

### Action Items

- [x] Add `EditOp::compile(&Context) -> Result<EditPlan, EditError>`:
  - [x] validate conflicts (multiple boundaries, contradictory cursor moves)
  - [x] normalize order (pre -> selection -> transform -> post)
  - [x] resolve policies (`UndoPolicy`/`SyntaxPolicy`) in one place
- [x] Make executor:
  - [x] compute plan (no mutation)
  - [x] call `doc.commit(commit)` once
  - [x] apply view changes based on `CommitResult` + post-effects
- [x] Delete "sprinkled save_undo_state calls" in transforms entirely

### Migration Notes

- You can keep current phased executor but route actual edits through commit
- Once stable, make compile stage pure for easy testing

### Remaining Work: edit_op_executor Full Migration ✅ COMPLETED

The `edit_op_executor.rs` migration has been completed. It now uses the compile->commit pattern
where transactions are built first and then applied via `apply_edit()` which handles undo
recording inside `commit()`.

**Previous state** (before migration):

#### Current State Analysis

**Current flow:**

```rust
// execute_edit_plan()
if needs_undo && plan.op.modifies_text() {
    self.save_undo_state();  // records undo BEFORE knowing the transaction
}
// pre-effects, selection ops...
self.apply_text_transform_with_plan(&plan);  // calls *_no_undo methods
// post-effects...
```

**Problems with current approach:**

1. Undo snapshot taken before transaction is known
2. Multiple `*_no_undo` helper methods duplicate the undo-recording versions
3. `apply_transaction_with_selection_inner()` duplicates `apply_transaction_inner()` from editing.rs
4. `PreEffect::SaveUndo` variant exists but shouldn't - undo is handled at plan level

#### Transform Categories

**Simple transforms** (single transaction):

- `TextTransform::Delete` - builds delete tx
- `TextTransform::Insert(text)` - builds insert tx
- `TextTransform::InsertNewlineWithIndent` - builds insert tx with computed indent
- `TextTransform::Deindent { max_spaces }` - builds delete tx

**Compound transforms** (multiple transactions composed):

- `TextTransform::Replace(text)` - delete tx + insert tx
- `TextTransform::MapChars(kind)` - delete tx + insert tx
- `TextTransform::ReplaceEachChar(ch)` - delete tx + insert tx

**Meta transforms** (no document undo needed):

- `TextTransform::Undo` - calls `self.undo()`
- `TextTransform::Redo` - calls `self.redo()`
- `TextTransform::None` - no-op

#### Migration Phases

**Phase 1: Refactor transforms to return transactions (not apply)**

Change `*_no_undo` methods to build and return transactions without applying:

```rust
// Before:
fn insert_text_no_undo(&mut self, text: &str) {
    let (tx, new_selection) = { ... };
    self.apply_transaction_with_selection_inner(buffer_id, &tx, Some(new_selection));
}

// After:
fn build_insert_transaction(&self, text: &str) -> Option<(Transaction, Selection)> {
    let buffer = self.buffer();
    Some(buffer.prepare_insert(text))
}
```

Methods refactored:

- [x] `insert_text_no_undo` → `build_insert_transaction`
- [x] `insert_newline_with_indent_no_undo` → `build_newline_with_indent_transaction`
- [x] `apply_char_mapping_no_undo` → `build_char_mapping_transaction`
- [x] `apply_replace_each_char_no_undo` → `build_replace_each_char_transaction`
- [x] `apply_deindent_no_undo` → `build_deindent_transaction`

**Phase 2: Add transaction composition for compound transforms**

For transforms that produce multiple transactions (delete + insert), compose them:

```rust
fn compose_transactions(txs: &[Transaction], text: RopeSlice) -> Transaction {
    txs.iter().fold(
        Transaction::new(text),
        |acc, tx| acc.compose(tx.clone()).expect("compose succeeds")
    )
}
```

Or alternatively, build a single transaction that does delete+insert in one pass:

```rust
// For Replace: build a single transaction that replaces each selection range
fn build_replace_transaction(&self, replacement: &str) -> Option<(Transaction, Selection)> {
    let buffer = self.buffer();
    buffer.with_doc(|doc| {
        let tx = Transaction::change_by_selection(
            doc.content().slice(..),
            &buffer.selection,
            |range| (range.from(), range.to(), Some(replacement.into()))
        );
        let new_sel = tx.map_selection(&buffer.selection);
        Some((tx, new_sel))
    })
}
```

**Phase 3: Update `apply_text_transform_with_plan` to collect transactions**

```rust
fn apply_text_transform_with_plan(&mut self, plan: &EditPlan) -> Option<(Transaction, Selection)> {
    match &plan.op.transform {
        TextTransform::None => None,
        TextTransform::Delete => self.build_delete_transaction(),
        TextTransform::Replace(text) => self.build_replace_transaction(text),
        TextTransform::Insert(text) => self.build_insert_transaction(text),
        TextTransform::InsertNewlineWithIndent => self.build_newline_with_indent_transaction(),
        TextTransform::MapChars(kind) => self.build_char_mapping_transaction(*kind),
        TextTransform::ReplaceEachChar(ch) => self.build_replace_each_char_transaction(*ch),
        TextTransform::Deindent { max_spaces } => self.build_deindent_transaction(*max_spaces),
        TextTransform::Undo => { self.undo(); None }
        TextTransform::Redo => { self.redo(); None }
    }
}
```

**Phase 4: Rewrite `execute_edit_plan` to use `apply_edit`**

```rust
pub fn execute_edit_plan(&mut self, plan: EditPlan) {
    // Check readonly before any mutation
    if plan.op.modifies_text() && !self.guard_readonly() {
        return;
    }

    // Pre-effects (yank, extend selection) - no undo recording here
    for pre in &plan.op.pre {
        self.apply_pre_effect(pre);
    }

    // Selection modification
    self.apply_selection_op(&plan.op.selection);

    let original_cursor = self.buffer().cursor;

    // Build and apply the transaction via apply_edit()
    if let Some((tx, new_selection)) = self.apply_text_transform_with_plan(&plan) {
        let buffer_id = self.focused_view();
        let origin = EditOrigin::EditOp { id: plan.op.id };
        self.apply_edit(buffer_id, &tx, Some(new_selection), plan.undo_policy, origin);
    }

    // Post-effects (mode change, cursor adjustment)
    for post in &plan.op.post {
        self.apply_post_effect(post, original_cursor);
    }
}
```

**Phase 5: Cleanup**

- [x] Remove `PreEffect::SaveUndo` variant (undo handled at plan level)
- [x] Delete `*_no_undo` methods
- [x] Delete `apply_transaction_with_selection_inner` (use `apply_transaction_inner`)
- [x] Update `replace_selection` to use the new pattern
- [ ] Remove `apply_transaction` public method if no longer needed (kept for external callers)

#### Note on Pre-Effects and Selection Ops

Pre-effects like `ExtendForwardIfEmpty` and selection ops modify the selection BEFORE the
transaction is built. This is correct - the transaction should use the modified selection.
The view snapshot for undo should capture the state BEFORE pre-effects too, which `apply_edit`
handles by capturing snapshots at the start.

#### capabilities.rs:138 (UndoAccess trait)

The `UndoAccess::save_state()` method is a public API for external callers (registry commands)
to manually record an undo boundary. This is a valid use case and should remain:

```rust
impl UndoAccess for Editor {
    fn save_state(&mut self) {
        self.save_undo_state();  // Keep this - external API
    }
}
```

However, `save_undo_state()` should be updated to only push `EditorUndoGroup` with view
snapshots and call `doc.record_undo_boundary()`. This is already done.

______________________________________________________________________

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

______________________________________________________________________

## Testing Strategy

### 1) Property Tests (High Value)

Use `proptest` (or `quickcheck`) on Transaction/undo invariants:

- [x] **Undo round-trip**: for random documents and random transactions, `apply tx`, then `apply tx.invert()` => document content returns to original
- [x] **Redo round-trip**: `apply tx`, undo, redo => content equals post-apply
- [x] **Selection mapping sanity** (if using mapped selections):
  - selection stays within bounds after apply/invert
  - mapping is stable across repeated operations
- [x] **Commit gate invariants**:
  - if readonly, commit returns `EditError::ReadOnly` and content unchanged
  - commit increments version exactly once per applied edit
  - redo stack clears on commit (when undo recorded)

If you keep snapshots temporarily, still property-test that undo/redo restores exact rope content.

### 2) Unit Tests for New Abstractions

- [x] `Document::commit`: sets modified, increments version, clears redo, records undo based on policy
- [x] `UndoStore` implementations:
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

______________________________________________________________________

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

______________________________________________________________________

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
