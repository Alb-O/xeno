# Async Hooks Refactor Plan (Definitive)

This document defines the finalized plan to complete async hook support needed for
LSP and other async extensions, while preserving Tome's orthogonal registry model.

## Decisions (v1)

- Keep sync entry points, but never drop async hooks. Sync emission schedules async
  hook futures onto a hook runtime.
- Cancelable operations (BufferWritePre, etc) must use async `emit().await`. Sync
  emission does not observe async cancellation.
- BufferChange uses full-document sync with a version counter in v1. Incremental
  changes are deferred.
- HookContext provides access to ExtensionMap for handle extraction. Async hooks
  must clone `Arc` handles before returning a future.
- Add `OwnedHookContext` + `to_owned()` for standard data cloning.
- High-frequency events (Tick, CursorMove, SelectionChange) use sync emission;
  extensions that need async work must debounce internally.
- Error handling: hook failures are logged and treated as Continue. No Error result
  in v1.
- No global hook cancellation/timeouts in v1; the LSP layer can enforce its own
  request timeouts.

## Current State (Baseline)

- `HookAction` supports Done/Async and `emit()` is async.
- `emit_sync()` exists but currently skips async hooks (will change).
- `open_buffer()` is async and `open_buffer_sync()` exists for sync callers.

## Design Overview

### Hook dispatch and scheduling

Introduce a scheduling hook runtime without coupling `tome-manifest` to `tome-api`.

```rust
pub trait HookScheduler {
    fn schedule(&mut self, fut: BoxFuture);
}

pub fn emit_sync_with(ctx: &HookContext<'_>, scheduler: &mut impl HookScheduler) -> HookResult {
    // Run sync hooks immediately, queue async hooks.
}
```

`tome-api` provides a runtime that implements `HookScheduler`:

```rust
pub struct HookRuntime {
    queue: VecDeque<BoxFuture>,
}

impl HookScheduler for HookRuntime {
    fn schedule(&mut self, fut: BoxFuture) {
        self.queue.push_back(fut);
    }
}

impl HookRuntime {
    pub async fn drain(&mut self) {
        while let Some(fut) = self.queue.pop_front() {
            let _ = fut.await;
        }
    }
}
```

Semantics:

- `emit()` awaits hooks sequentially (preserves ordering, supports cancel).
- `emit_sync_with()` runs sync hooks immediately, queues async hooks in priority
  order, and returns based only on sync results.
- `emit_sync()` remains for tests or isolated contexts and explicitly skips async
  hooks.

The main loop drains `HookRuntime` once per tick (or after each event batch) so
queued async work runs promptly without blocking sync callers.

### Hook context ownership and services

Extend `HookContext` variants to carry `extensions: &ExtensionMap`. Async hooks
extract clonable handles before returning a future:

```rust
let lsp = ctx.extensions.get::<Arc<LspManager>>().cloned();
let owned = ctx.to_owned();
HookAction::Async(Box::pin(async move {
    if let (Some(lsp), OwnedHookContext::BufferOpen { path, text, file_type, .. }) =
        (lsp, owned)
    {
        let _ = lsp.did_open(&path, &text, file_type.as_deref()).await;
    }
    HookResult::Continue
}))
```

Add `OwnedHookContext` + `to_owned()` for buffer/file data. It does not carry
ExtensionMap; async hooks must clone what they need up front.

### BufferChange strategy (v1)

Full-document sync, versioned:

- Add `version: u64` to Buffer and increment on every transaction.
- Track dirty buffers in `Editor` to avoid scanning all buffers each tick.
- Emit `BufferChange` with full text + version from tick via `emit_sync_with()`.

Example:

```rust
self.buffers[id].version += 1;
self.dirty_buffers.insert(id);
```

```rust
for id in self.dirty_buffers.drain() {
    let buffer = self.buffers.get(id)?;
    if let Some(path) = &buffer.path {
        emit_sync_with(
            &HookContext::BufferChange {
                path,
                version: buffer.version,
                text: buffer.doc.slice(..),
                extensions: &self.extensions,
            },
            &mut self.hook_runtime,
        );
    }
}
```

Incremental changes and range payloads are deferred.

## Phase 1: Hook runtime + delivery semantics

1. Add `HookScheduler` trait + `emit_sync_with` to `tome-manifest`.
2. Add `HookRuntime` to `tome-api` and store it in `Editor`.
3. Update sync call sites to use `emit_sync_with`.
4. Drain `HookRuntime` from the main loop so queued async hooks run.

## Phase 2: Missing hook emissions

- `EditorStart`/`EditorQuit` emitted via a scope guard in `run_editor` so quit fires
  on early returns.
- `EditorTick` uses `emit_sync_with`.
- `BufferClose` emitted when buffers are removed.
- `WindowResize`, `FocusGained`, `FocusLost` emit sync.
- `CursorMove`/`SelectionChange` emit only on change.

## Phase 3: BufferChange (full sync)

- Add `version` to Buffer.
- Mark dirty buffers at editor edit entry points.
- Emit `BufferChange` from tick via `emit_sync_with`.

## Phase 4: HookContext ownership + services

- Add `extensions` to HookContext variants.
- Add `OwnedHookContext` and `to_owned()`.
- Update hook macro docs/examples to show clone-before-async pattern.

## Phase 5: LSP hooks

- Store `Arc<LspManager>` in ExtensionMap.
- Hooks: BufferOpen/Change/Write/Close.
- Use full-document `didChange` with version.
- Errors logged, never panic.

## Phase 6: Async trait refactor (deferred)

- Revisit async `BufferOpsAccess` only if required after the scheduler is in place.
- Keep scope tight to avoid widespread async propagation in v1.

## Phase 7: Testing

- Unit tests for `emit_sync_with` scheduling and ordering.
- Tests for BufferChange full sync and version increments.
- LSP integration tests (kitty harness): didOpen, didChange, didClose.

## Implementation Order

1. HookScheduler + HookRuntime + emit_sync_with.
2. EditorStart/Quit + tick + window hooks.
3. BufferChange full sync + versioning + dirty queue.
4. HookContext extensions + OwnedHookContext.
5. LSP hooks.
6. Cursor/selection hooks if not already done.
7. Tests.

## Deferred / v2

- Incremental BufferChange (per-transaction ranges).
- Hook cancellation/timeouts.
- Concurrent async hook execution while preserving priority semantics.
- Async trait refactor if sync APIs become a bottleneck.

## Files to Modify

| File | Changes |
|------|---------|
| `crates/manifest/src/hooks.rs` | HookScheduler trait, emit_sync_with, OwnedHookContext, HookContext extensions |
| `crates/manifest/src/lib.rs` | Re-export new hook API |
| `crates/api/src/editor/mod.rs` | HookRuntime integration, dirty queue, hook emissions |
| `crates/api/src/editor/input_handling.rs` | Cursor/selection/mode hook emission |
| `crates/api/src/buffer/editing.rs` | Buffer version increment |
| `crates/term/src/app.rs` | EditorStart/Quit with scope guard |
| `crates/extensions/extensions/lsp/` | LSP hooks using Arc handles |

## Alternatives (Not Chosen)

- Channel-based async worker: adds indirection and breaks ordering guarantees for
  cancelable hooks.
- All hooks async: unnecessary overhead for hot paths and sync-only hooks.
- Separate sync/async registries: complicates ordering and does not solve sync
  emission needs.
- Poll-based hooks: manual state machines and poor Tokio integration.
