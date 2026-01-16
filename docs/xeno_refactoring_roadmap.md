# Xeno scale-management refactoring roadmap (post-Phases 1–8)

**Scope:** reduce policy duplication, tighten invariants, and make future growth (crates/features) cheaper.

## Current baseline (verified)
- `Editor::apply_edit` uses **prepare → apply → finalize** via `UndoManager` and a clean `UndoHost` boundary.
- Undo is **two-layer**: document text history vs editor view-state history.
- `invocation.rs` is the correct unified pipeline, but not yet the only entrypoint.

## Guiding invariants (non-negotiable)
1. **Single mutation gate:** all text mutation goes through `Document::commit(EditCommit)`.
2. **Single invocation gate:** all user-triggered execution goes through `invocation.rs`.
3. **No raw lock guards escape:** no `.read()`/`.write()` on document locks outside the narrowest internal layer; locks must not be held across UI/LSP/hook boundaries.
4. **No policy re-derivation:** capability enforcement, readonly enforcement, undo grouping, and syntax dirtiness decisions must not be reimplemented at call sites.

---

## Answers to the inline questions (so you can proceed confidently)

### 1) Should legacy `execute_action` delegate to `run_action_invocation`?
**Yes.** Make the legacy path a thin wrapper that **calls the same core pipeline** (cap checks + pre/post hooks + apply effects) and then delete duplicated checks/hook emission.

Implementation tip: keep a **shared, non-async “core” function** that both async and sync call sites can use:
- `run_action_invocation_async(...)` wraps hook scheduling and awaits.
- `run_action_invocation_core(...)` does the deterministic work (lookup → caps/readonly checks → handler → apply effects).

### 2) EditorCommands lack capability metadata — migrate or extend?
You have two good options; I recommend **Option A** (unifies the world), but you can stage via Option B.

- **Option A (preferred):** move EditorCommands into the same registry shape as Commands (with `required_caps`), using a handler variant like `CommandHandler::Editor(fn(&mut Editor, ...))`.
- **Option B (staged):** add `required_caps: CapabilitySet` to EditorCommand definitions and enforce it in `run_editor_command_invocation` immediately; later migrate to the registry.

### 3) Is `mem::take` around `undo_manager` acceptable?
It works, but it’s a **code smell at scale** because it spreads an awkward pattern everywhere.

Cleaner approach: give `UndoManager` a single helper that owns the prepare/finalize dance and accepts a closure to apply the edit:

```rust
impl UndoManager {
    pub fn with_edit<H, F>(
        &mut self,
        host: &mut H,
        buffer_id: BufferId,
        undo: UndoPolicy,
        origin: EditOrigin,
        apply: F,
    ) -> bool
    where
        H: UndoHost,
        F: FnOnce(&mut H) -> bool,
    {
        let prep = self.prepare_edit(host, buffer_id, undo, origin);
        let applied = apply(host);
        self.finalize_edit(applied, prep);
        applied
    }

    pub fn with_undo_redo<H, F>(&mut self, host: &mut H, f: F)
    where
        H: UndoHost,
        F: FnOnce(&mut UndoManager, &mut H),
    {
        f(self, host)
    }
}
```

Then `Editor::apply_edit/undo/redo` can be written without `mem::take`.

---

# Multi-phased plan (actionable, checkboxed)

## Phase 1 — Invocation becomes the only entrypoint
**Goal:** one capability gate, one hook emission pipeline, one readonly gate, one place to interpret policy.

- [ ] **Inventory** all call sites that execute actions/commands outside `invocation.rs` (e.g. legacy `actions_exec.rs`, tests, macro replay, LSP-triggered edits).
- [ ] Create a small public API surface on `Editor`:
  - [ ] `invoke_action(...) -> InvocationResult`
  - [ ] `invoke_command(...) -> InvocationResult`
- [ ] Refactor legacy `execute_action` to delegate to invocation:
  - [ ] Replace duplicated capability check + hook emission with a call into invocation.
  - [ ] Mark legacy function `#[deprecated]`.
  - [ ] Delete duplicated check blocks once call sites are migrated.
- [ ] Bring `run_editor_command_invocation` into parity:
  - [ ] Ensure it returns the same `InvocationResult` variants.
  - [ ] Ensure readonly enforcement is consistent.
- [ ] Tests:
  - [ ] Action with required caps is blocked when `enforce_caps=true`.
  - [ ] Same action logs a warning but runs when `enforce_caps=false`.
  - [ ] Pre/post hooks fire exactly once per invocation.

### Code sketch: unify legacy + new invocation paths
```rust
pub fn execute_action_legacy(&mut self, name: &str) -> bool {
    // thin wrapper; no policy here
    matches!(
        futures::executor::block_on(self.run_invocation(Invocation::Action(name.into()), InvocationPolicy::default())),
        InvocationResult::Applied
    )
}
```
(If you avoid `block_on` in the TUI thread, expose a sync `run_action_invocation_core` and call it directly.)

---

## Phase 2 — Capability metadata for EditorCommands
**Goal:** remove the “third” capability world.

- [ ] **Pick staging strategy**:
  - [ ] **A:** migrate EditorCommands into the registry `CommandDef` model.
  - [ ] **B:** add `required_caps` to EditorCommand and enforce it in invocation immediately.
- [ ] Implement enforcement for EditorCommands:
  - [ ] Add `required_caps` field and plumb it to `run_editor_command_invocation`.
  - [ ] Use the exact same `check_all_capabilities(...)` logic and policy interpretation.
- [ ] Optional (recommended): unify display/diagnostics:
  - [ ] A single “capability denied” notification path.
  - [ ] A single place to render “why unavailable” (caps vs readonly).

### Code sketch: staged EditorCommand metadata
```rust
pub struct EditorCommandDef {
    pub name: SmolStr,
    pub required_caps: CapabilitySet,
    pub handler: fn(&mut Editor, EditorCommandArgs) -> EditorCommandResult,
}
```

---

## Phase 3 — Remove `mem::take` by giving `UndoManager` a closure helper
**Goal:** keep the prepare/finalize discipline, stop scattering ownership hacks.

- [ ] Add `UndoManager::with_edit(...)` as shown above.
- [ ] Rewrite `Editor::apply_edit` to:
  - [ ] Call `self.core.undo_manager.with_edit(self, ...)`.
  - [ ] Move `apply_transaction_inner` call into the closure.
- [ ] Rewrite `Editor::undo/redo` similarly (either with `with_undo_redo` or direct calls if borrow checker permits).
- [ ] Tests:
  - [ ] No behavior change (golden tests / snapshot tests for undo grouping).
  - [ ] Ensure `finalize_edit` runs even when apply fails.

---

## Phase 4 — Enforce document lock hygiene
**Goal:** make “closure-scoped lock access” the *default and enforced* pattern.

- [ ] Mechanical refactor: convert Buffer methods that use `.read()/.write()` to `with_doc/with_doc_mut`.
- [ ] Reduce escape hatches:
  - [ ] Make `Buffer::document_arc()` `pub(crate)` or remove it from the public API.
  - [ ] Restrict any remaining raw lock access to a tiny internal module.
- [ ] Optional but powerful: introduce a `DocumentHandle` newtype that hides the lock.
- [ ] Guardrail in CI:
  - [ ] Add a simple `rg "\.read\(\)|\.write\(\)" crates/editor/src | ...` check that fails unless the file path is allowlisted.

### Code sketch: `DocumentHandle` wrapper
```rust
#[derive(Clone)]
pub struct DocumentHandle(Arc<RwLock<Document>>);

impl DocumentHandle {
    pub fn with<R>(&self, f: impl FnOnce(&Document) -> R) -> R {
        let g = self.0.read().unwrap();
        f(&g)
    }

    pub fn with_mut<R>(&self, f: impl FnOnce(&mut Document) -> R) -> R {
        let mut g = self.0.write().unwrap();
        f(&mut g)
    }
}
```

---

## Phase 5 — Remove high-risk mutation footguns
**Goal:** eliminate APIs that silently bypass undo/syntax/version invariants.

- [ ] Restrict `Document::content_mut()` visibility (`pub(crate)` or `#[cfg(test)]`).
- [ ] Migrate call sites to one of:
  - [ ] `Document::commit(EditCommit { ... })` (preferred)
  - [ ] `Document::reset_content(...)` (for bulk replaces)
- [ ] Add tests:
  - [ ] “bulk replace” invalidates incremental syntax and triggers full reparse.
  - [ ] undo history behavior is explicit (either cleared or recorded as a reset step).

---

## Phase 6 — Make `CommitResult` the single source of truth for downstream decisions
**Goal:** stop re-deriving “what happened” after commit (undo grouping, syntax dirtiness, LSP sync).  
Even if commit is already the gate, enriching/using `CommitResult` will remove duplicated logic.

- [ ] Expand `CommitResult` to include all downstream-relevant signals.
- [ ] Update `Buffer::apply*` to consume `CommitResult` instead of probing document state again.
- [ ] Update UndoManager/editor grouping decisions to be driven from `CommitResult` (when possible).

### Code sketch: recommended `EditCommit / CommitResult / EditError` shape
```rust
pub struct EditCommit {
    pub tx: Transaction,
    pub undo: UndoPolicy,
    pub syntax: SyntaxPolicy,
    pub origin: EditOrigin,
}

pub struct CommitResult {
    pub applied: bool,
    pub undo_recorded: bool,
    pub insert_group_active_after: bool,
    pub changed_ranges: SmallVec<[Range<usize>; 2]>,
    pub syntax_outcome: SyntaxOutcome,
}

pub enum EditError {
    Readonly,
    InvalidTransaction,
    InvariantViolation(&'static str),
}
```

---

## Phase 7 — Slim the Buffer edit API to one canonical entrypoint
**Goal:** one way to apply a transaction, with policy explicitly passed.

- [ ] Choose the canonical method: `Buffer::apply(tx, ApplyPolicy, loader)`.
- [ ] Convert other variants (`apply_with_lsp`, “with undo”, “with syntax”) into thin wrappers.
- [ ] Deprecate legacy variants; delete after call sites migrate.

---

## Phase 8 — Testing and verification (keep regressions near zero)

### Unit tests (fast)
- [ ] Capability gating matrix: enforce vs log-only, and readonly enforcement.
- [ ] Invocation pipeline: pre/post hooks exactly-once, errors propagate predictably.
- [ ] UndoManager: prepare/finalize called in all cases (success/failure).

### Property tests (high value)
- [ ] **Edit/undo/redo roundtrip:** applying random transaction sequences followed by undo/redo returns to identical rope + selection state.
- [ ] **Undo group boundaries:** random insert streams should group as expected; non-insert edits break groups.

### Integration tests
- [ ] LSP incremental sync correctness against a reference apply path (feature-gated).
- [ ] Multi-buffer same-document: sibling selection sync after apply.

### CI guardrails
- [ ] “No raw lock access” grep/lint.
- [ ] “No legacy invocation” grep: forbid new call sites to `actions_exec` once Phase 1 is complete.

---

# Migration checklist (incremental adoption without breaking the editor)

1. **Phase 1 first:** route new features and call sites through invocation; keep legacy wrappers temporarily.
2. **Phase 3 early:** remove `mem::take` to simplify future refactors and reduce cognitive load.
3. **Phase 4 continuously:** mechanical lock hygiene refactor can be done in small PRs.
4. **Phase 5–7 next:** tighten APIs and remove footguns once call sites are centralized.
5. **Phase 8 always:** add tests as you delete duplication.


---

# Appendix A — Key type sketches (for consistency and future diffs)

## A1) UndoPolicy / SyntaxPolicy / ApplyPolicy
```rust
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum UndoPolicy {
    /// Do not record undo; does not interact with insert-group state.
    None,

    /// Record undo and merge into the current insert group if allowed.
    InsertMerge,

    /// Record undo and force a group boundary.
    NewGroup,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SyntaxPolicy {
    /// Apply incremental edits if possible; otherwise mark dirty for full reparse.
    IncrementalOrDirty,

    /// Force full reparse/dirtiness (bulk changes, uncertain diffs).
    ForceDirty,

    /// Do not touch syntax state (tests / special tools only; avoid in prod paths).
    None,
}

pub struct ApplyPolicy {
    pub undo: UndoPolicy,
    pub syntax: SyntaxPolicy,
}
```

## A2) EditCommit / CommitResult / EditError
> Even if these exist today, keep one canonical definition (and keep it small). The goal is: downstream code never has to re-derive policy outcomes.

```rust
pub struct EditCommit {
    pub tx: Transaction,
    pub undo: UndoPolicy,
    pub syntax: SyntaxPolicy,
    pub origin: EditOrigin,
}

pub enum SyntaxOutcome {
    IncrementalApplied,
    MarkedDirty,
    Unchanged,
}

pub struct CommitResult {
    pub applied: bool,
    pub undo_recorded: bool,
    pub insert_group_active_after: bool,
    pub changed_ranges: SmallVec<[Range<usize>; 2]>,
    pub syntax_outcome: SyntaxOutcome,
}

pub enum EditError {
    Readonly,
    InvalidTransaction,
    InvariantViolation(&'static str),
}
```

## A3) UndoStore trait (optional, but great for scale)
If you expect to keep multiple document undo backends (snapshot vs transaction), hide them behind a single interface so Document code never branches on backend type.

```rust
pub trait UndoStore {
    type Step;

    fn record_step(&mut self, step: Self::Step);
    fn undo(&mut self) -> Option<Self::Step>;
    fn redo(&mut self) -> Option<Self::Step>;
    fn clear_redo(&mut self);
}

pub struct DocumentUndoStep {
    pub before: Rope,
    pub after: Rope,
    pub changed_ranges: SmallVec<[Range<usize>; 2]>,
}
```

## A4) DocumentUndoStep vs EditorUndoGroup boundary
Keep the separation explicit. Document undo contains only **text state** (and maybe doc-local metadata). Editor undo groups contain **view state** and a list of affected documents.

```rust
pub struct EditorUndoGroup {
    pub docs: SmallVec<[DocumentId; 2]>,
    pub views: HashMap<BufferId, ViewSnapshot>,
}
```

## A5) Closure-based lock access (canonical pattern)
```rust
impl Buffer {
    pub fn with_doc<R>(&self, f: impl FnOnce(&Document) -> R) -> R {
        self.document.with(|doc| f(doc))
    }

    pub fn with_doc_mut<R>(&mut self, f: impl FnOnce(&mut Document) -> R) -> R {
        self.document.with_mut(|doc| f(doc))
    }
}
```

---

# Appendix B — Phase gates (what must be true before moving on)

## Gate after Phase 1
- All interactive action execution flows through invocation (legacy wrapper only).
- Exactly one capability gate and one hook emission per invocation.

## Gate after Phase 2
- EditorCommands and Commands share the same capability enforcement semantics.

## Gate after Phase 3
- No `mem::take` usage to work around borrow issues in undo-related call sites.

## Gate after Phase 4
- No raw `.read()`/`.write()` on Document locks outside an allowlisted module.

## Gate after Phase 6
- Downstream systems consume `CommitResult` instead of probing doc state to infer outcomes.
