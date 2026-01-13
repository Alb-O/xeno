# LSP Development Strategy for Xeno

> Notes from collaborative discussion with GPT-5.2 Thinking (January 2026)
> Based on review of `lsp-codemap.md` and core editor source files.

## Table of Contents

### Strategy & Design
1. [Architecture Assessment](#architecture-assessment)
2. [Critical Improvements Needed](#critical-improvements-needed)
3. [Feature Priorities](#feature-priorities)
4. [WorkspaceEdit Applier Design](#workspaceedit-applier-design)
5. [Undo/Redo Strategy](#undoredo-strategy)
6. [LSP Sync Architecture](#lsp-sync-architecture)
7. [Completion UI Design](#completion-ui-design)

### Implementation Checklist
8. [Implementation Checklist](#implementation-checklist) (with phase gates inline)
   - [Phase 1: Foundation](#phase-1-foundation)
   - [Phase 2: WorkspaceEdit Engine](#phase-2-workspaceedit-engine)
   - [Milestone: Code Actions + Rename](#milestone-ship-code-actions--rename)
   - [Phase 3: Completion System](#phase-3-completion-system)
   - [Phase 4: Resilience](#phase-4-resilience)
   - [Phase 5: Polish](#phase-5-polish--additional-features)
9. [Dependency Graph](#dependency-graph)
10. [Suggested Order](#suggested-order)
11. [Risk Assessment](#risk-assessment)
12. [Testing Strategy](#testing-strategy)

### Appendices
13. [Key Code Locations](#appendix-key-code-locations)
14. [Patterns to Keep](#appendix-patterns-to-keep)
15. [Anti-Patterns to Fix](#appendix-anti-patterns-to-fix)

---

## Architecture Assessment

### What Looks Strong

1. **Clean layering + decoupling**
   - Split between `Registry` (server lifecycle), `DocumentSync` (didOpen/didChange/etc), and event handlers
   - The "one-liner" constructor `DocumentSync::create` wires events correctly

2. **Server lifecycle is practical and restart-aware**
   - `Registry::get_or_start` explicitly detects crashed servers and restarts them
   - Keyed by `(language, root)` with root markers

3. **Position encoding correctness**
   - UTF-8/16/32 offset conversions with tests
   - This unglamorous work is critical for editor reliability

4. **Diagnostics plumbing is editor-friendly**
   - `DocumentStateManager` has global `diagnostics_version` counter
   - Supports project-wide diagnostics by creating state on demand

5. **Key capability hooks in place**
   - Server-side router handles `workspace/configuration` and `window/workDoneProgress/create`
   - Unhandled notifications don't kill the loop

6. **Command layer already wired**
   - Hover + goto-definition connected to editor commands
   - Pattern ready for code actions / rename / symbols

---

## Critical Improvements Needed

### 1. Crash Recovery Must Re-open Documents Automatically

**Problem:**
- `Registry` restarts dead server on next `get_or_start` call
- But `DocumentSync::notify_change_full` uses `registry.get(...)` (no restart)
- Still bumps document version even if there's no live client

**Net effect:** If server crashes, edits silently stop syncing until user triggers path that calls `get_or_start` again.

**Fix direction:**
```rust
// In notify_change_*, if registry.get(...) is None:
// 1. Call get_or_start(...)
// 2. Re-send didOpen for that document (using current text + current version)
// Already have machinery: open_document calls get_or_start and sends didOpen
```

### 2. Incremental Sync Must Not "Fail Into No-Op"

**Problem:**
- `compute_lsp_changes` returns `Vec::new()` as failure escape hatch when range conversion fails
- `notify_change_incremental` returns early on empty changes
- Combination can desync buffer without any obvious error signal

**Fix direction:**
```rust
// Make incremental computation return Result or enum:
enum IncrementalResult {
    Incremental(Vec<LspDocumentChange>),
    FallbackToFull,  // When conversion fails
}
// Then fallback to full sync rather than "send nothing"
```

### 3. Make Full vs Incremental an Internal Policy

**Problem:**
- `LspManager` exposes full-sync and incremental paths separately
- Editor shouldn't need to choose

**Fix direction:**
```rust
// Single entry point:
impl LspManager {
    pub async fn on_buffer_change(&mut self, buffer: &Buffer) {
        // Internally decide: if server supports incremental && changes valid
        // → send incremental; otherwise → send full
    }
}
```

### 4. Bounded Queues + Timeouts + Cancellation

**Need:**
- Per-request timeouts
- `$/cancelRequest` for cursor-driven requests (hover/completion/signatureHelp) when cursor moves
- Prevents "late results" from fighting the UI

### 5. Memory Hygiene for Project-Wide Diagnostics

**Problem:**
- Creating document state on-demand for diagnostics is useful
- But needs eviction policy for big repos

**Fix direction:**
- LRU/TTL eviction
- Or: "only keep diagnostics for open docs + last N closed docs"

---

## Feature Priorities

### Tier 1: "Must Feel Like an IDE" (Highest ROI)

| Feature | Notes |
|---------|-------|
| **Diagnostics UX** | Gutter/signs, underline ranges, quick list, jump next/prev. Already have event stream and counts. |
| **Code Actions (quick-fix)** | UI to show actions for cursor/range; requires robust WorkspaceEdit applier |
| **Rename** | Same WorkspaceEdit engine as code actions. First feature forcing multi-file edits correctness. |
| **Completion Polish** | Already have server config for snippet support. Need: completion UI, snippet insertion, debounce/cancel. |

### Tier 2: "Productivity Boosters"

| Feature | Notes |
|---------|-------|
| **Signature Help** | Especially useful in Rust |
| **Document Symbols / Outline** | Already have protocol call |
| **Workspace Symbols** | Global search |
| **Format on Save** | Build from formatting + save hooks |

### Tier 3: "Nice-to-Have / Expensive"

| Feature | Notes |
|---------|-------|
| **Inlay Hints** | Nice but optional with Tree-sitter |
| **Semantic Tokens** | Optional unless want semantic-level theming |
| **Code Lens** | Lower priority |

---

## WorkspaceEdit Applier Design

### Core Principle: Two-Phase Application

1. **Plan/validate** (no mutations): resolve target docs, convert LSP ranges → CharIdx, build per-buffer edit plans, detect version mismatches
2. **Apply atomically** (mutate): push undo state, apply edits, bump versions, trigger reparse, enqueue LSP sync

This avoids half-applied multi-file edits.

### Data Structures

```rust
/// A validated, ready-to-apply workspace edit
struct WorkspaceEditPlan {
    per_buffer: Vec<BufferEditPlan>,
}

struct BufferEditPlan {
    buffer_id: BufferId,
    /// Edits in CharIdx space on the current Rope (non-overlapping, sorted)
    edits: Vec<PlannedTextEdit>,
    /// True if buffer was opened just for this edit (may close after)
    opened_temporarily: bool,
}

struct PlannedTextEdit {
    range: std::ops::Range<CharIdx>,
    replacement: Tendril,  // or String
}
```

### Resolution Steps

1. **For each URI in WorkspaceEdit:**
   - If already open → map to `buffer_id`
   - If not open → open "headless" buffer via existing `open_file`/`open_buffer`
   - Mark `opened_temporarily = true` (optional policy: close after apply)

2. **Convert ranges correctly:**
   - Convert LSP Position (line/character in encoding) → byte/char offset
   - Convert to CharIdx in the Rope
   - Do all conversions against same document snapshot

3. **Handle version mismatches:**
   - **Strict**: refuse and show error ("document changed; retry")
   - **Best-effort**: re-plan against latest doc right before apply (recommended)

4. **Sort and validate edits:**
   - Sort by (start, end) ascending
   - Validate non-overlap after conversion
   - Build `Transaction::change(doc, changes_iter)` and apply

### Entry Point

```rust
impl Editor {
    /// Apply a WorkspaceEdit from LSP (code action, rename, etc.)
    pub async fn apply_workspace_edit(&mut self, edit: WorkspaceEdit) -> Result<(), ApplyError> {
        // 1. Plan
        let plan = self.plan_workspace_edit(edit)?;
        
        // 2. Enter edit group (push undo for all affected buffers)
        self.begin_workspace_edit_group(&plan);
        
        // 3. Apply all buffer mutations
        for buffer_plan in &plan.per_buffer {
            self.apply_buffer_edit_plan(buffer_plan)?;
        }
        
        // 4. Flush LSP notifications immediately
        self.flush_lsp_sync_now(&plan.affected_buffer_ids());
        
        Ok(())
    }
}
```

---

## Undo/Redo Strategy

### Current State

- **Snapshot-based**: clones entire `Rope` for each undo entry
- **Per-document**: `Document.undo_stack` / `redo_stack`
- **Insert grouping**: consecutive insert-mode edits grouped via `insert_undo_active` flag

```rust
pub struct HistoryEntry {
    doc: Rope,  // Full clone of document
    selections: HashMap<BufferId, Selection>,
}
```

### Recommendation: Keep Snapshots + Add Editor-Level Grouping

**Why keep snapshots:**
- `ropey::Rope::clone()` is cheap (structural sharing)
- Cost is mostly in modifications that fork nodes
- Real problem is semantic structure, not memory

**Add editor-level grouping for multi-file edits:**

```rust
/// Editor-level undo entry for grouping multi-buffer edits
enum EditorUndoEntry {
    /// Single buffer edit (delegates to document undo)
    Single { buffer_id: BufferId },
    /// Grouped edit across multiple buffers (WorkspaceEdit, rename, etc.)
    Group { buffers: Vec<BufferId> },
}

impl Editor {
    undo_group_stack: Vec<EditorUndoEntry>,
}
```

**On WorkspaceEdit apply:**
1. For each affected buffer: `push_undo_snapshot(...)`
2. Push one `EditorUndoEntry::Group { buffers }`

**On undo:**
1. Pop group entry
2. Call `doc.undo(...)` for each buffer in reverse order

### Future: Transaction-Based History (Parallel Path)

Start recording transactions for new edit sources while leaving typing history snapshot-based:

```rust
enum HistoryEntry {
    Snapshot { doc: Rope, selections: HashMap<BufferId, Selection> },
    Transaction { 
        inverse: Transaction, 
        selections_before: HashMap<BufferId, Selection>,
        selections_after: HashMap<BufferId, Selection>,
    },
}
```

Benefits:
- Merge adjacent typing transactions
- Keep history lightweight
- Multi-file edits as single `HistoryEntry::Group(Vec<(BufferId, inverse_txn)>)`

---

## LSP Sync Architecture

### Current Flow

```
User types → insert_text() → apply_transaction_with_selection()
                                      ↓
                           apply_edit_with_lsp()
                                      ↓
                     compute_lsp_changes() → pending_lsp_changes.extend()
                                      ↓
                           dirty_buffers.insert()
                                      ↓
                              tick() runs
                                      ↓
                           queue_lsp_change()
                                      ↓
                    tokio::spawn(notify_change_incremental/full)
```

### Problem with WorkspaceEdit

If we just use `dirty_buffers`:
1. Apply all edits → insert into `dirty_buffers`
2. Wait for next `tick()` to flush
3. **One tick of latency** before server knows about changes

### Recommended Architecture

**Two flushing modes:**

| Mode | Trigger | Behavior |
|------|---------|----------|
| **Tick batching** | Typing / normal edits | Cheap, coalesces bursts |
| **Flush-now** | WorkspaceEdit / rename / code action / completion | Immediate, no tick latency |

**Add per-document sync state:**

```rust
pub struct Document {
    // ... existing fields ...
    
    /// Pending incremental changes
    pub pending_lsp_changes: Vec<LspDocumentChange>,
    
    /// Force full sync on next flush (set by undo/redo, WorkspaceEdit)
    pub force_full_sync: bool,
}
```

**Modified queue_lsp_change:**

```rust
fn queue_lsp_change(&mut self, buffer_id: BufferId) {
    let buffer = self.buffers.get_buffer(buffer_id)?;
    let doc = buffer.doc();
    
    // Early return if nothing to send
    if doc.pending_lsp_changes.is_empty() && !doc.force_full_sync {
        return;
    }
    
    let use_full = doc.force_full_sync || /* existing fallback conditions */;
    doc.force_full_sync = false;  // Reset flag
    
    // ... dispatch notification ...
}
```

**Flush-now API:**

```rust
impl Editor {
    /// Immediately flush LSP sync for specified buffers
    /// Returns handle that can optionally be awaited for ordering guarantees
    pub fn flush_lsp_sync_now(&mut self, buffer_ids: &[BufferId]) -> FlushHandle {
        let mut handles = Vec::new();
        
        for &buffer_id in buffer_ids {
            if let Some(handle) = self.queue_lsp_change_immediate(buffer_id) {
                handles.push(handle);
            }
        }
        
        FlushHandle { handles }
    }
}

pub struct FlushHandle {
    handles: Vec<oneshot::Receiver<()>>,
}

impl FlushHandle {
    /// Wait until all didChange messages have been written to outbound stream
    pub async fn await_synced(self) {
        for handle in self.handles {
            let _ = handle.await;
        }
    }
}
```

### Critical: Serialize LSP Outbound Traffic Per Client

**Problem:**
- Current `tokio::spawn` per `queue_lsp_change()` can cause:
  - Reorderings between didChange messages
  - Interleavings with requests
  - Concurrent writes

**Solution: One outbound dispatcher task per LSP client**

```rust
struct ClientHandle {
    outbound_tx: mpsc::Sender<OutboundMsg>,
    // ...
}

enum OutboundMsg {
    Notification { method: String, params: Value },
    Request { id: RequestId, method: String, params: Value },
    /// Optional ack fired after write completes
    DidChange { params: Value, ack: Option<oneshot::Sender<()>> },
}

// Single task reads OutboundMsgs and writes JSON-RPC messages sequentially
async fn outbound_dispatcher(mut rx: mpsc::Receiver<OutboundMsg>, writer: impl AsyncWrite) {
    while let Some(msg) = rx.recv().await {
        // Write JSON-RPC message
        write_message(&writer, &msg).await;
        
        // Fire ack if present
        if let OutboundMsg::DidChange { ack: Some(tx), .. } = msg {
            let _ = tx.send(());
        }
    }
}
```

---

## Completion UI Design

### Controller Structure

```rust
pub struct CompletionController {
    /// Monotonically increasing generation for stale detection
    generation: u64,
    /// Currently in-flight request, if any
    in_flight: Option<InFlightCompletion>,
}

struct InFlightCompletion {
    gen: u64,
    cancel: tokio_util::sync::CancellationToken,
    /// LSP request ID if server supports $/cancelRequest
    request_id: Option<RequestId>,
}
```

### Trigger Rules

| Trigger | Debounce |
|---------|----------|
| Typing | 50-120ms |
| Explicit (Ctrl-Space) | None |
| Cursor move | Cancel if popup open, or retrigger with debounce |

### Execution Flow

```rust
impl CompletionController {
    pub fn trigger(&mut self, ctx: CompletionContext) {
        // 1. Increment generation
        self.generation += 1;
        let gen = self.generation;
        
        // 2. Cancel previous request
        if let Some(in_flight) = self.in_flight.take() {
            in_flight.cancel.cancel();
            // Optionally send $/cancelRequest
        }
        
        // 3. Spawn new request
        let cancel = CancellationToken::new();
        self.in_flight = Some(InFlightCompletion { gen, cancel: cancel.clone(), .. });
        
        tokio::spawn(async move {
            // Debounce
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(DEBOUNCE_DURATION) => {}
            }
            
            // Snapshot state
            let snapshot = ctx.snapshot();
            
            // Send request
            let result = ctx.client.completion(snapshot.params).await;
            
            // Check staleness before rendering
            if gen != ctx.current_generation() { return; }
            if snapshot.doc_version != ctx.current_doc_version() { return; }
            if snapshot.cursor_pos != ctx.current_cursor_pos() { return; }
            
            // Render popup
            ctx.render_completion_menu(result);
        });
    }
}
```

### Applying Completion Items

Completion application is a mini WorkspaceEdit:
- `CompletionItem.textEdit` OR `insertText`
- Plus `additionalTextEdits`
- Plus optional `command`

**Reuse the WorkspaceEdit applier:**

```rust
impl Editor {
    pub fn apply_completion_item(&mut self, item: CompletionItem) {
        // Convert to WorkspaceEditPlan (single buffer most of the time)
        let plan = self.plan_completion_edit(item);
        
        // Apply as undo group (even within one buffer)
        self.begin_workspace_edit_group(&plan);
        self.apply_buffer_edit_plan(&plan);
        
        // If snippet: parse and expand placeholders
        if item.insert_text_format == InsertTextFormat::Snippet {
            self.expand_snippet(&item.insert_text);
        }
        
        // Execute command if present
        if let Some(cmd) = item.command {
            self.execute_lsp_command(cmd);
        }
    }
}
```

---

## Implementation Checklist

> Checklist-oriented task breakdown with acceptance criteria and phase gates.

### Quick Wins (Do Anytime)

| Task | Impact | Notes |
|------|--------|-------|
| **P1-T2** Early-return in `queue_lsp_change` | High | Prevents accidental full-sync spam |
| **P4-T2** Incremental failure → fallback-to-full | High | Eliminates silent desync class |
| **P4-T1** Crash recovery auto-reopen | High | Big reliability win immediately |
| **P5-T1** Diagnostics navigation | Medium | User-visible value, minimal protocol work |

---

### Phase 1: Foundation

**Goal:** Make LSP output ordering deterministic and make "flush-now" safe.

#### Tasks

- [x] **P1-T1: Add `Document.force_full_sync` field**
  - Add `force_full_sync: bool` to Document struct
  - Set by undo/redo path and WorkspaceEdit path
  - Clear `pending_lsp_changes` when set
  - **Acceptance:** Undo/redo sets flag and clears incremental changes

- [x] **P1-T2: Early-return in `queue_lsp_change`**
  - Return without spawning if `pending_lsp_changes.is_empty() && !force_full_sync`
  - **Acceptance:** Tick does not trigger full-sync when changes already flushed

- [x] **P1-T3: Define `OutboundMsg` enum + channel on `ClientHandle`**
  - Add `outbound_tx: mpsc::Sender<OutboundMsg>` to `ClientHandle`
  - Define `OutboundMsg::{Notification, Request, DidChange{ack}}`
  - **Acceptance:** Unit test enqueues two messages, observes FIFO order

- [x] **P1-T4: Implement per-client outbound dispatcher task**
  - One async task per client reads channel, writes sequentially
  - Optional ack fired after write completes
  - **Acceptance:** Concurrent callers cannot interleave writes

- [x] **P1-T5: Route all send APIs through outbound channel**
  - Migrate existing `notify_*` / request methods
  - **Acceptance:** Hover/goto-definition still works; no direct-write paths remain

- [x] **P1-T6: Implement `flush_lsp_sync_now(buffer_ids) -> FlushHandle`**
  - Triggers sync immediately without waiting for tick
  - Returns optional receivers for "written" barrier
  - **Acceptance:** Can flush and optionally await for ordering-sensitive flows

#### Phase 1 Gate

**Must be true before proceeding:**
- [x] All outbound LSP messages serialized per client (no concurrent writes)
- [x] `queue_lsp_change` does not full-sync when nothing pending unless forced
- [x] `flush_lsp_sync_now` works without causing extra sync on next tick

**Verification:**
- Integration test with fake JSON-RPC server asserting message order
- Stress test: spam didChange + hover requests; confirm ordered stream

---

### Phase 2: WorkspaceEdit Engine

**Goal:** Reusable multi-file edit applier for code actions/rename/completion edits.

#### Tasks

- [x] **P2-T1: Add `EditorUndoEntry::Group` + wiring**
  - Editor-level stack entry grouping buffer IDs
  - `undo()` applies grouped doc undos in reverse
  - **Acceptance:** Grouped undo restores all affected buffers in one action

- [x] **P2-T2: Create `WorkspaceEditPlan` structs**
  - Define `WorkspaceEditPlan`, `BufferEditPlan`, `PlannedTextEdit`
  - **Acceptance:** Can construct plan by hand in unit test

- [x] **P2-T3: Implement URI→buffer resolution**
  - Helper: `resolve_uri_to_buffer(uri) -> (buffer_id, opened_temporarily)`
  - Open headless buffers for unopened files
  - **Acceptance:** Can plan edits for files not currently open

- [x] **P2-T4: Implement range conversion (LSP → CharIdx)**
  - `convert_text_edit(rope, encoding, lsp_edit) -> PlannedTextEdit`
  - **Acceptance:** Unit tests pass for UTF-8/16/32 encodings

- [x] **P2-T5: Sort/validate/coalesce edits**
  - Stable sort by position; detect overlaps; coalesce adjacent
  - **Acceptance:** Overlaps produce clear `ApplyError`

- [x] **P2-T6: Build Transaction + apply to Rope**
  - `apply_buffer_edit_plan(buffer_id, edits)` → transaction, syntax update, version bump
  - **Acceptance:** Multi-edit yields correct text; `doc.version` increments once

- [x] **P2-T7: Implement `apply_workspace_edit` end-to-end**
  - Full flow: plan → begin group → apply all → set `force_full_sync` → flush
  - **Acceptance:** Multi-file edit atomic; one undo reverts all; server receives didChange

#### Phase 2 Gate

**Must be true before proceeding:**
- [x] Can apply multi-file WorkspaceEdit atomically
- [x] Single undo reverts all touched buffers
- [x] Flush-now sends didChange immediately for touched docs

**Verification:**
- Integration: fake server sends WorkspaceEdit across 2-3 files
- Property test: random non-overlapping edits validate correct final text

---

### Milestone: Ship Code Actions + Rename

After Phase 2, these features can ship:

- [x] **P5-T2: Code Actions UI MVP**
  - Request actions at cursor/range via LSP
  - Show picker/menu for selection
  - Apply via WorkspaceEdit engine
  - **Acceptance:** Quick-fix applies and is undoable atomically

- [x] **P5-T3: Rename MVP**
  - Prompt UI for new name
  - Request rename via LSP
  - Apply via WorkspaceEdit engine
  - **Acceptance:** Multi-file rename applies; single undo reverts all

---

### Phase 3: Completion System

**Goal:** Completion that is responsive, cancellable, and uses the edit engine.

#### Tasks

- [x] **P3-T1: Implement `CompletionController` + generation tracking**
  - Controller struct with `generation: u64`, `in_flight: Option<InFlight>`
  - **Acceptance:** Trigger twice → generation increments, prior marked stale

- [x] **P3-T2: Add cancellation + debounce flow**
  - Debounce timer; cancellation token; optional `$/cancelRequest`
  - **Acceptance:** Integration test: only latest results render; late responses ignored

- [x] **P3-T3: Completion menu UI**
  - Anchor at cursor; show/hide; selection movement; accept/cancel
  - **Acceptance:** Ctrl-Space opens; typing updates; escape closes

- [x] **P3-T4: Apply completion via WorkspaceEdit engine**
  - Convert `textEdit` + `additionalTextEdits` to plan
  - **Acceptance:** Completion with additional edits is atomic + undoable

- [x] **P3-T5: Snippet insertion MVP**
  - Handle `$0`, `$1..$n` placeholders (basic)
  - **Acceptance:** Common rust-analyzer snippets work; unsupported falls back to plain text

#### Phase 3 Gate

**Must be true before proceeding:**
- [x] Debounce/cancel works; stale results never render
- [x] Applying completion with additional edits is atomic + undoable

**Verification:**
- Integration: delayed server responses; only latest generation renders
- Manual: rapid typing doesn't flicker; escape closes

---

### Phase 4: Resilience

**Goal:** Never silently desync; survive restarts cleanly.

#### Tasks

- [x] **P4-T1: Crash recovery auto-reopen**
  - Detect missing client in `notify_change_*`
  - Call `get_or_start()` + resend `didOpen` with current text/version
  - **Acceptance:** Kill server mid-edit → auto-reopen, syncing continues

- [x] **P4-T2: Incremental failure → fallback-to-full**
  - Return `Result` or enum from `compute_lsp_changes`
  - Callers send full sync on failure
  - **Acceptance:** Conversion failure never results in "no didChange sent"

- [x] **P4-T3: Request timeouts + cancellation**
  - Timeout wrapper for hover/completion/signatureHelp
  - Cancel inflight on cursor move
  - **Acceptance:** Stalled server doesn't hang UI; clean timeout error

#### Phase 4 Gate

**Must be true before proceeding:**
- [x] Server crash triggers auto-reopen and syncing continues
- [x] Incremental conversion failure always falls back to full
- [x] Requests time out; cancels don't leak tasks

**Verification:**
- Integration: kill fake server mid-session; confirm reopen
- Unit: forced conversion failure returns fallback, not empty

---

### Phase 5: Polish + Additional Features

- [x] **P5-T1: Diagnostics UX MVP**
  - Gutter signs for diagnostic severity
  - Underline ranges in buffer
  - Jump next/prev diagnostic commands
  - **Acceptance:** Diagnostics visible; navigation works

- [x] **P5-T4: Signature Help**
  - Trigger on `(` character
  - Display parameter info popup
  - **Acceptance:** Shows signature on function call

---

## Dependency Graph

```
P1-T3 → P1-T4 → P1-T5 (dispatcher chain)
     ↘
P1-T1 → P1-T2 → P1-T6 (sync control chain)
                   ↓
              P2-T1 (undo grouping)
                   ↓
        P2-T2 → P2-T3 → P2-T4 → P2-T5 → P2-T6 → P2-T7 (WorkspaceEdit chain)
                                                   ↓
                                    ┌──────────────┼──────────────┐
                                    ↓              ↓              ↓
                                 P3-T4          P5-T2          P5-T3
                               (completion)  (code actions)   (rename)
```

### Critical Path (to Rename + Code Actions)

1. P1-T3 → P1-T5 (dispatcher)
2. P1-T1 → P1-T2 (sync control)
3. P1-T6 (flush-now)
4. P2-T1 (undo grouping)
5. P2-T2 → P2-T7 (WorkspaceEdit chain)
6. P5-T2 / P5-T3 (code actions + rename)

Completion (Phase 3) can run in parallel after P2-T7.

---

## Suggested Order

```
Phase 1: Foundation
├── P1-T3 → P1-T4 → P1-T5 (dispatcher)
├── P1-T1 → P1-T2 (sync control)
└── P1-T6 (flush-now)

Phase 2: WorkspaceEdit
├── P2-T1 (undo grouping)
└── P2-T2 → P2-T7 (full chain)

Milestone: Ship First Features
├── P5-T2 (Code Actions MVP)
└── P5-T3 (Rename MVP)

Phase 3: Completion
└── P3-T1 → P3-T5 (full chain)

Phase 4: Resilience
├── P4-T1 (crash recovery)
├── P4-T2 (fallback-to-full)
└── P4-T3 (timeouts)

Phase 5: Polish
├── P5-T1 (Diagnostics UX)
└── P5-T4 (Signature help)
```

---

## Risk Assessment

| Risk | Mitigation | Fallback |
|------|------------|----------|
| **Outbound serialization retrofit** - hidden direct-write callsites | Audit all send paths; compile-time guards | Panic in debug on concurrent/direct writes |
| **Range conversion edge cases** - encoding/line-ending issues | Extensive unit tests | Force full sync + descending edits |
| **Headless buffer semantics** - hooks/UI assumptions | Mark buffers "not visible"; skip UI hooks | Keep open normally; close later |
| **Undo grouping correctness** - buffer destroyed between apply/undo | Validate buffer existence | Best-effort; warn if missing |

---

## Testing Strategy

### Outbound Dispatcher
- **Unit:** FIFO ordering, ack fires after write, channel closure
- **Integration:** Fake server records sequence; assert no interleaving
- **Manual:** Stress typing + hover + completion

### queue_lsp_change + flush-now
- **Unit:** Early-return logic; forced-full resets flag
- **Integration:** Flush-now then tick: no second full sync
- **Manual:** Code action then hover—verify updated text

### WorkspaceEdit Planner/Applier
- **Unit:** Overlap detection; coalescing; encoding conversions
- **Integration:** Multi-file edit; undo group revert
- **Manual:** Rename across files in real project

### Completion Controller
- **Unit:** Generation monotonicity; stale-drop logic
- **Integration:** Delayed responses; cancel previous
- **Manual:** Rapid typing; menu stability

### Resilience
- **Integration:** Crash/restart path; fallback-to-full; timeout
- **Manual:** Kill LSP server; keep editing; verify recovery

---

## Appendix: Key Code Locations

| Component | Location |
|-----------|----------|
| Transaction model | `crates/base/src/transaction/mod.rs` |
| ChangeSet (OT) | `crates/base/src/transaction/changeset.rs` |
| Document (undo/redo) | `crates/api/src/buffer/document.rs` |
| Buffer editing | `crates/api/src/buffer/editing.rs` |
| Editor editing | `crates/api/src/editor/editing.rs` |
| LSP change computation | `crates/lsp/src/changes.rs` |
| LSP sync dispatch | `crates/api/src/editor/lifecycle.rs` |
| Info popup | `crates/api/src/info_popup.rs` |
| Main entry point | `crates/term/src/main.rs` |

---

## Appendix: Patterns to Keep

1. **Event-handler interface for server→client events**
   - Great seam for "showMessage" UI, progress indicators, logs

2. **Central conversion utilities with tests**
   - Keep expanding tests as new edit-application code lands

3. **Command layer pattern**
   - Wire new LSP features (code actions, rename, symbols) the same way as hover/goto-definition

---

## Appendix: Anti-Patterns to Fix

1. **Silent drop of sync updates**
   - Incremental empty vector + missing server = silent desync
   - Fix: fallback to full sync, never silently skip

2. **Version increments when nothing delivered**
   - If keeping monotonic versions, guarantee future `didOpen` includes latest version/text after restart

3. **Exposed ordering in LSP outbound**
   - Multiple `tokio::spawn` can interleave messages
   - Fix: serialize per client via channel + dispatcher

