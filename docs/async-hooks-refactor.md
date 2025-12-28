# Async Hooks Refactor Plan (Definitive)

This document defines the finalized plan to complete async hook support needed for
LSP and other async extensions, while preserving Tome's orthogonal registry model.

## Implementation Status

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 1: Hook runtime | ✅ Complete | `HookScheduler`, `emit_sync_with`, `HookRuntime` |
| Phase 2: Hook emissions | ✅ Complete | All lifecycle hooks except CursorMove/SelectionChange |
| Phase 3: BufferChange | ✅ Complete | Version tracking, dirty buffer queue |
| Phase 4: HookContext services | ✅ Complete | Type-erased `extensions` field via `dyn Any` |
| Phase 5: LSP hooks | ✅ Complete | `LspManager` + hooks for open/change/close/quit |
| Phase 6: Async trait refactor | ⏸ Deferred | Not needed for v1 |
| Phase 7: Testing | ⏸ Partial | Unit tests exist, integration tests pending |

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

## Architecture

### HookContext Structure

`HookContext` is a struct with two fields:
- `data: HookEventData<'a>` - The event-specific payload (enum)
- `extensions: Option<&'a dyn Any>` - Type-erased access to `ExtensionMap`

This design:
- Avoids duplicating `extensions` in every enum variant
- Allows `tome-manifest` to remain decoupled from `tome-api` (uses `dyn Any`)
- Hooks downcast via `ctx.extensions::<ExtensionMap>()`

### Hook dispatch and scheduling

`tome-manifest` defines the scheduler trait:

```rust
pub trait HookScheduler {
    fn schedule(&mut self, fut: BoxFuture);
}

pub fn emit_sync_with(ctx: &HookContext<'_>, scheduler: &mut impl HookScheduler) -> HookResult {
    // Run sync hooks immediately, queue async hooks.
}
```

`tome-api` provides the runtime:

```rust
pub struct HookRuntime {
    queue: VecDeque<BoxFuture>,
}

impl HookScheduler for HookRuntime { ... }

impl HookRuntime {
    pub async fn drain(&mut self) { ... }
}
```

Semantics:

- `emit()` awaits hooks sequentially (preserves ordering, supports cancel).
- `emit_sync_with()` runs sync hooks immediately, queues async hooks in priority
  order, and returns based only on sync results.
- `emit_sync()` remains for tests or isolated contexts and explicitly skips async
  hooks.

The main loop drains `HookRuntime` once per tick so queued async work runs promptly.

### Hook context ownership and services

Async hooks extract clonable handles before returning a future:

```rust
fn my_hook_handler(ctx: &HookContext) -> HookAction {
    let lsp = ctx.extensions::<ExtensionMap>()
        .and_then(|ext| ext.get::<Arc<LspManager>>())
        .cloned();
    let owned = ctx.to_owned();
    
    let Some(lsp) = lsp else { return HookAction::done() };
    
    HookAction::Async(Box::pin(async move {
        if let OwnedHookContext::BufferOpen { path, text, file_type } = owned {
            lsp.did_open(&path, &text, file_type.as_deref(), 1).await;
        }
        HookResult::Continue
    }))
}
```

### LSP Integration

The `lsp` extension (`crates/extensions/extensions/lsp/`) provides:

- `LspManager`: Wraps `Registry`, stored as `Arc<LspManager>` in `ExtensionMap`
- Hooks registered for: `BufferOpen`, `BufferChange`, `BufferClose`, `EditorQuit`
- Default language server configs for: rust, typescript, javascript, python, go

## Files Modified

| File | Changes |
|------|---------|
| `crates/manifest/src/hooks.rs` | `HookContext` struct with `data`+`extensions`, `HookEventData` enum, `HookScheduler` trait, `emit_sync_with` |
| `crates/manifest/src/lib.rs` | Re-export `HookEventData` |
| `crates/api/src/editor/mod.rs` | `HookRuntime` field, dirty buffer queue, hook emissions with extensions |
| `crates/api/src/editor/hook_runtime.rs` | `HookRuntime` implementation |
| `crates/api/src/editor/input_handling.rs` | Mode change hook emission |
| `crates/api/src/buffer/editing.rs` | Buffer version increment |
| `crates/term/src/app.rs` | `EditorStart`/`EditorQuit` hooks |
| `crates/extensions/extensions/lsp/mod.rs` | `LspManager`, extension init |
| `crates/extensions/extensions/lsp/hooks.rs` | LSP hooks |
| `crates/stdlib/src/hooks/*.rs` | Updated to use `ctx.data` pattern |

## Deferred / v2

- Incremental BufferChange (per-transaction ranges).
- CursorMove/SelectionChange hooks (need change detection).
- Hook cancellation/timeouts.
- Concurrent async hook execution while preserving priority semantics.
- Async trait refactor if sync APIs become a bottleneck.

## Alternatives (Not Chosen)

- Channel-based async worker: adds indirection and breaks ordering guarantees for
  cancelable hooks.
- All hooks async: unnecessary overhead for hot paths and sync-only hooks.
- Separate sync/async registries: complicates ordering and does not solve sync
  emission needs.
- Poll-based hooks: manual state machines and poor Tokio integration.
- `ExtensionMap` in `tome-manifest`: would break layered architecture (manifest is
  definitions only).
