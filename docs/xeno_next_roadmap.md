# Xeno Next Roadmap: Runtime + LSP Sync Hardening

> **Status:** Phases 1–8 (core architecture refactor) are complete.
>
> **This roadmap** targets the next scaling bottlenecks: async hook execution and LSP sync efficiency/correctness.

---

## 0. Overview

### 0.1 What’s already done (Phases 1–8 recap)

The foundation is now solid and enforceable:

- **Single invocation gate:** all user execution goes through `invocation.rs`.
- **Single mutation gate:** all text mutation goes through `Document::commit(EditCommit)`.
- **Two-layer undo:** document undo + editor grouping via `UndoHost`, with a safe `UndoManager::with_edit(...)` closure helper.
- **Lock hygiene:** `DocumentHandle` + `with_doc/with_doc_mut` closures; CI guardrails prevent raw lock access.
- **No mutation footguns:** restricted `content_mut` and standardized reset paths.
- **`CommitResult` is authoritative:** downstream decisions driven from `CommitResult` (undo grouping, syntax outcomes, changed ranges).
- **Slim Buffer API:** canonical `Buffer::apply(tx, ApplyPolicy, loader)`.
- **Test coverage + CI:** capability gating matrix, invocation exactly-once hooks, undo safety, property/integration tests.

### 0.2 Goals of this next phase

You’ve reached the point where correctness is mostly about **time and concurrency**, not just structure.

This roadmap focuses on:

1. **Eliminating UI stalls** caused by awaiting async hooks inline.
2. **Making LSP sync efficient** (debounced, low-clone, low-spam) and **correct** (version discipline + recovery).
3. **Adding observability** so future regressions are obvious and diagnosable.

### 0.3 Target invariants (new)

Add these to the existing invariant set:

1. **No “await-all” in the render loop:** the main loop must not block on unbounded async work.
2. **Bounded per-tick background work:** hooks + LSP flush each have explicit time budgets.
3. **Incremental LSP does not require full document cloning.**
4. **LSP version discipline:** LSP versions advance **only when messages are sent**, and stale work is rejected.

---

## Phase A — Hook Runtime Hardening

### Goals

- Remove UI jank caused by `HookRuntime::drain().await` awaiting all pending futures.
- Allow hooks to run concurrently.
- Add a per-tick budget for hook completions.

### Success criteria

- The main loop never blocks longer than the configured hook budget.
- Pending hook depth does not grow without bound under normal use.
- Hook failures do not crash the main loop; failures are logged with context.

### Tasks

#### A1 — Switch from FIFO "await-all" to concurrent execution

- [x] Replace `VecDeque<HookBoxFuture>` with `FuturesUnordered<HookBoxFuture>` (or equivalent).
- [x] Keep `HookScheduler::schedule(fut)` unchanged for call sites.
- [x] Add `HookRuntime::pending_count()` and `HookRuntime::has_pending()` backed by `running.len()`.

**Code sketch (core runtime):**

```rust
use futures::stream::{FuturesUnordered, StreamExt};
use xeno_registry::{BoxFuture as HookBoxFuture, HookScheduler};

pub struct HookRuntime {
    running: FuturesUnordered<HookBoxFuture>,
    // Optional: stats for instrumentation
    scheduled_total: u64,
    completed_total: u64,
}

impl Default for HookRuntime {
    fn default() -> Self {
        Self {
            running: FuturesUnordered::new(),
            scheduled_total: 0,
            completed_total: 0,
        }
    }
}

impl HookScheduler for HookRuntime {
    fn schedule(&mut self, fut: HookBoxFuture) {
        self.running.push(fut);
        self.scheduled_total += 1;
    }
}
```

#### A2 — Add budgeted draining (poll completions only within a time budget)

- [x] Implement `HookRuntime::drain_budget(budget: Duration)`.
- [x] Ensure `drain_budget` returns promptly when:
  - no hooks are pending,
  - budget is exhausted,
  - no hook completes within remaining time.

**Code sketch (budgeted drain):**

```rust
impl HookRuntime {
    pub async fn drain_budget(&mut self, budget: std::time::Duration) {
        let deadline = std::time::Instant::now() + budget;

        while std::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());

            // Wait for at most `remaining` for a single completion.
            let next = tokio::time::timeout(remaining, self.running.next()).await;
            match next {
                Ok(Some(_)) => {
                    self.completed_total += 1;
                }
                _ => break, // timeout, empty stream, or no progress
            }
        }
    }

    pub fn pending_count(&self) -> usize {
        self.running.len()
    }
}
```

#### A3 — Wire budgets into the main loop

- [x] Replace `editor.hook_runtime.drain().await` with `drain_budget(...)`.
- [x] Choose budgets that preserve responsiveness:
  - **Fast redraw (16ms):** 1–2ms budget
  - **Slow redraw (50ms):** 3–5ms budget
- [x] Add a debug log when pending depth exceeds a high-water mark.

**Code sketch (main loop):**

```rust
let hook_budget = if matches!(editor.mode(), Mode::Insert) {
    std::time::Duration::from_millis(1)
} else {
    std::time::Duration::from_millis(3)
};

editor.hook_runtime.drain_budget(hook_budget).await;

if editor.hook_runtime.pending_count() > 500 {
    tracing::warn!(pending = editor.hook_runtime.pending_count(), "hook backlog high");
}
```

#### A4 — Backpressure safety valve (minimal)

- [x] Define a high-water mark (e.g., 500–2,000 depending on expected load).
- [x] If exceeded:
  - [x] log a warning with queue depth and last few hook types (if available),
  - [ ] optionally drop **non-critical** hooks (Phase E can refine).

### Phase gate

You can move to Phase B when:

- [x] Rendering remains responsive even when hooks are slow.
- [x] `HookRuntime::drain_budget` is used in the main loop.
- [x] There is a visible warning mechanism for sustained hook backlog.

---

## Phase B — LSP Debounce + Efficiency

### Goals

- Avoid sending LSP notifications every tick / every keystroke.
- Remove unnecessary cloning of full document content for incremental sync.
- Send at most one LSP notification per document per debounce window.

### Success criteria

- Incremental typing does not produce an LSP notify per tick.
- Incremental sync does not clone full `Rope` content.
- Full sync occurs only when forced (fallback, threshold, desync recovery).

### Tasks

#### B1 — Add pending LSP accumulator per document

- [x] Add `pending_lsp: HashMap<DocumentId, PendingLsp>` to editor state (or LSP manager).
- [x] When a buffer is dirty, drain its doc's pending LSP changes and append into `PendingLsp`.

**Data model sketch:**

```rust
struct PendingLsp {
    last_edit_at: std::time::Instant,
    force_full: bool,
    changes: Vec<LspDocumentChange>,
    bytes: usize,
    // Optional: store editor doc version snapshot for version discipline (Phase C)
    editor_version: u64,
}
```

#### B2 — Implement tick-based debounce flush

- [x] Choose a debounce target (start with **30ms**).
- [x] Add `flush_lsp_pending(now)` at end of `Editor::tick()`.
- [x] Flush conditions:
  - [x] `now - last_edit_at >= debounce`, OR
  - [x] thresholds exceeded, OR
  - [x] `force_full == true`.

**Flush logic sketch (single send per doc):**

```rust
const LSP_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(30);

fn flush_lsp_pending(&mut self, now: Instant) {
    for (doc_id, pending) in self.pending_lsp.iter_mut() {
        let due = now.duration_since(pending.last_edit_at) >= LSP_DEBOUNCE;
        if !(due || pending.force_full) {
            continue;
        }

        // Pull required info (path, language) from a representative buffer/document mapping
        // and send (incremental or full).
        // On success: clear pending.
        // On error: set pending.force_full = true and keep pending for retry.
    }
}
```

> **Note:** This approach avoids cancellation tokens and per-doc spawned tasks. It’s deterministic and easy to reason about.

#### B3 — Remove full content clone for incremental sync

Today you clone content even for incremental sends. Replace with one of:

**Preferred:** change `DocumentSync` API:

- [x] Update `notify_change_incremental` to not require `content`.
- [x] Keep `notify_change_full` requiring full text.

**API sketch:**

```rust
async fn notify_change_incremental(
    &self,
    path: &Path,
    language: &FileType,
    changes: Vec<LspDocumentChange>,
) -> Result<()>;

async fn notify_change_full(
    &self,
    path: &Path,
    language: &FileType,
    content: Rope,
) -> Result<()>;
```

**Fallback option:** lazy provider:

- [x] Replace `content: Rope` parameter with `content_provider: impl FnOnce() -> Rope`.
- [x] Only call provider on full sync.

#### B4 — Keep existing thresholds, apply them at the accumulator level

You already have:

- `LSP_MAX_INCREMENTAL_CHANGES`
- `LSP_MAX_INCREMENTAL_BYTES`

Move the threshold logic to the pending accumulator:

- [x] As changes are appended, track total change count and bytes.
- [x] If thresholds exceeded, set `force_full = true`.

### Phase gate

You can move to Phase C when:

- [x] LSP sends are debounced and no longer fire per tick while typing.
- [x] Incremental sync does not clone the full document content.
- [x] Threshold-triggered fallback to full sync still works correctly.

---

## Phase C — LSP Version Discipline + Recovery

### Goals

- Make LSP versioning explicit and correct.
- Prevent stale incremental edits from being sent.
- Provide recovery path when incremental sync fails or desync is detected.

### Success criteria

- LSP `DocumentState.version` increments exactly once per `didChange` send.
- Stale queued work is dropped or converted to full sync.
- On error, the system converges back to a correct state (eventually full sync).

### Tasks

#### C1 — Separate "editor version" from "LSP message version"

- [x] Treat editor `doc.version()` as internal monotonic truth.
- [x] Treat LSP `DocumentState.version()` as "messages sent" counter.

Add per-document tracking (in your sync layer):

- [x] `last_sent_editor_version: u64` (atomic or lock-protected).

#### C2 — Attach editor version to pending flush

- [x] Record `PendingLsp.editor_version` at accumulation time.
- [x] On flush, read current editor version:
  - [x] if `current_editor_version != pending.editor_version` and pending is old, either:
    - drop stale pending and rebuild from current changes, or
    - force full sync.

Rule of thumb:
- If you can guarantee your pending buffer always accumulates in order and you don’t send concurrently per doc, you can typically send the latest pending without special handling.
- If you allow concurrent sends or overlapping tasks, you must detect and reject stale sends.

#### C3 — Make sends per-document single-flight

- [x] Ensure at most one in-flight send per document.
- [x] If a send is in-flight and new edits arrive:
  - [x] accumulate new edits in pending,
  - [x] next flush occurs after the in-flight send completes.

Implementation options:
- Keep sending on the tick thread (no `tokio::spawn`), awaiting with a short budget and resuming later (requires async-aware tick), OR
- Keep `tokio::spawn`, but track in-flight state per doc and avoid starting another send.

**Minimal approach (recommended for now):**
- Continue using `tokio::spawn` for sends,
- Add `in_flight: bool` (or a `JoinHandle`) per doc,
- If `in_flight`, skip flush for that doc.

#### C4 — Error recovery forces full

- [x] On any send error:
  - [x] log with doc/path/language,
  - [x] set `force_full = true`,
  - [x] schedule retry after a backoff (e.g., 250ms).

#### C5 — Desync detection / full resync triggers

Add full sync triggers when:

- [x] incremental conversion fails (`FallbackToFull`),
- [x] thresholds exceeded,
- [x] server rejects an edit / returns an error indicating mismatch,
- [x] document open state changes or language ID changes.

### Phase gate

You can move to Phase D when:

- [x] There are no overlapping sends per document.
- [x] Errors reliably force full sync and the system recovers.
- [x] Version counters are coherent and exercised by tests.

---

## Phase D — Observability (Tracing + Metrics)

### Goals

- Make performance and correctness regressions visible immediately.
- Provide a causal chain from keypress → commit → pending accumulation → send.

### Success criteria

- You can answer “why did the UI jank?” from logs.
- You can answer “why did LSP desync?” from logs.

### Tasks

#### D1 — Hook tracing

- [x] Emit `hook.schedule` events (hook name/id, pending count).
- [x] Emit `hook.complete` events (duration, success/failure).
- [x] Include budget info during drain: `hook.drain_budget` (budget_ms, completed_count, pending_after).

#### D2 — LSP tracing

- [x] `lsp.pending_append` (doc id, added_changes, bytes, force_full).
- [x] `lsp.flush_start` (doc id, mode=inc/full, changes_count, bytes).
- [x] `lsp.flush_done` (latency_ms, success/failure, lsp_version, editor_version).

#### D3 — Lightweight metrics (optional but recommended)

- [x] gauge: `hooks.pending`
- [x] gauge: `lsp.pending_docs`
- [x] counter: `lsp.full_sync`
- [x] counter: `lsp.incremental_sync`
- [x] counter: `lsp.send_errors`

#### D4 — Debug command / overlay

- [x] Add a debug command (or statusline detail) to dump:
  - [x] hook pending count
  - [x] pending LSP docs
  - [x] per-doc pending change counts

### Phase gate

You can move to Phase E when:

- [x] Hook and LSP activity is visible in tracing.
- [x] There is at least one "user-facing" way to inspect backlog state.

---

## Phase E — Optional Refinements

These are valuable but should follow the core hardening.

### E1 — Coalescing (after debounce)

**Goal:** reduce LSP payload size and number of edits.

- [ ] Coalesce at flush time (operate on `PendingLsp.changes`).
- [ ] Merge adjacent deletes.
- [ ] Merge consecutive inserts at same point.
- [ ] Convert delete+insert at same range to replace.

**Gate:** payload sizes decrease measurably under sustained typing.

### E2 — Hook priorities

**Goal:** prevent low-value hooks from starving interactive behavior.

- [ ] Introduce `HookPriority { Interactive, Background }`.
- [ ] Allow drop policy for Background hooks under backlog.
- [ ] Optionally preserve ordering only for Interactive hooks.

### E3 — Unify async work under a scheduler abstraction

**Goal:** a single backpressure system for hooks, LSP, indexing, watchers.

- [ ] Introduce `Scheduler` trait and route hooks + LSP flush through it.
- [ ] Per-tick budgets and priorities become explicit.
- [ ] Add cancellation by `(doc_id, kind)`.

### E4 — Test expansions

- [ ] HookRuntime:
  - [ ] budgeted drain does not block beyond budget (use tokio time control)
  - [ ] backlog warnings trigger
- [ ] LSP:
  - [ ] debounce sends at most one notification per window
  - [ ] incremental path does not clone full content
  - [ ] error forces full and recovers
  - [ ] no overlapping sends per doc
- [ ] Optional: coalescing property tests

---

## Suggested Implementation Order (fastest value)

1. **Phase A** (HookRuntime concurrent + budgeted drain) — removes UI stalls.
2. **Phase B** (LSP pending + debounce + no incremental clone) — reduces CPU/network churn.
3. **Phase C** (version + single-flight + recovery) — prevents long-tail correctness bugs.
4. **Phase D** (tracing + basic metrics) — prevents regressions and shortens debugging.
5. **Phase E** optional refinements.

---

## Checklist Summary

- [x] Phase A: Hooks concurrent + budgeted drain + backlog safety
- [x] Phase B: LSP pending accumulator + debounce + eliminate incremental content clone
- [x] Phase C: LSP version discipline + single-flight + error recovery
- [x] Phase D: Tracing spans + minimal metrics + debug inspection
- [ ] Phase E: Coalescing + hook priorities + unified scheduler + expanded tests
