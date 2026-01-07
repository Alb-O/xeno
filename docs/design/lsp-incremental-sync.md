# LSP Incremental Document Sync

**Status**: Implemented (incremental when supported; full sync fallback)  
**Author**: Design session  
**Date**: 2026-01-08  
**Last Updated**: 2026-01-08

## Problem Statement

LSP's `textDocument/didChange` notification supports two sync modes:

1. **Full sync**: Send entire document content on every change
2. **Incremental sync**: Send only the changed range + replacement text

We previously used full sync exclusively. The incremental sync API existed but was broken: it
received post-change text but needed to compute positions in pre-change text.

### Why This Matters

| File Size | Full Sync Cost | Incremental Cost |
|-----------|----------------|------------------|
| 1k lines  | ~50KB/change   | ~100 bytes       |
| 10k lines | ~500KB/change  | ~100 bytes       |
| 50k lines | ~2.5MB/change  | ~100 bytes       |

For large files, full sync creates measurable latency for:
- Completion popup responsiveness
- Signature help updates
- Diagnostic refresh
- General typing feel

## Previous Bug (Fixed)

The old incremental API computed LSP ranges from the post-change rope, so positions were
shifted whenever edits occurred before the reported range.

## Current Architecture (Implemented)

```
┌─────────────────────────────────────────────────────────────┐
│                        Editor                                │
│                                                              │
│  ┌──────────┐    ┌─────────────┐    ┌──────────────────┐   │
│  │  Buffer  │───▶│ Transaction │───▶│ Document (Rope)  │   │
│  │  .edit() │    │   .apply()  │    │                  │   │
│  └──────────┘    └─────────────┘    └──────────────────┘   │
│                         │                                    │
│                         ▼                                    │
│            compute LSP changes (pre-change)                  │
│                         │                                    │
│                         ▼                                    │
│          queue LSP changes on Document                       │
│                         │                                    │
│                         ▼                                    │
│               ┌─────────────────┐                           │
│               │  tick() loop    │                           │
│               │  - emit hook    │                           │
│               │  - drain LSP    │◀── pre-computed ranges    │
│               └─────────────────┘                           │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
              ┌───────────────────────┐
              │   DocumentSync        │
              │   .notify_change_full │  ◀── fallback
              │   .notify_change_v2   │  ◀── incremental
              └───────────────────────┘
```

## Implemented Solution

### Types (Actual)

```rust
// In xeno-base

/// A pre-computed LSP document change, ready to send.
#[derive(Debug, Clone)]
pub struct LspDocumentChange {
    /// The range in the document that was replaced (pre-change positions).
    pub range: LspRange,
    /// The text that replaced the range.
    pub new_text: String,
}

#[derive(Debug, Clone, Copy)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}
```

`LspChangeSet` exists for future batching metadata but is not currently used.

### Integration Summary (as built)

- **xeno-base**: Added LSP range/position/change types; exposed `Transaction::operations()`.
- **xeno-lsp**: Added `compute_lsp_changes()` and tests; added `DocumentSync::notify_change_incremental_v2`.
- **xeno-api**: Added `Buffer::apply_edit_with_lsp()` and document change queue; updated edit paths to use
  incremental when supported; tick drains queued changes and sends incremental or full sync.

Key files:
- `crates/base/src/lsp.rs`
- `crates/base/src/transaction/mod.rs`
- `crates/lsp/src/changes.rs`
- `crates/lsp/src/sync.rs`
- `crates/api/src/buffer/editing.rs`
- `crates/api/src/buffer/document.rs`
- `crates/api/src/editor/editing.rs`
- `crates/api/src/editor/lifecycle.rs`
- `crates/api/src/lsp.rs`

### Behavior Details

1. **Edit time**: If the LSP client supports incremental sync, compute LSP ranges against the
   pre-change rope and queue them on the Document.
2. **Apply**: Apply the transaction and update syntax as usual.
3. **Tick**: Drain pending changes per document and send one `didChange` with ordered changes.
4. **Fallback**: If incremental is unsupported or unavailable, send a full sync.

## Testing (Implemented)

- Unit tests for change computation in `crates/lsp/src/changes.rs`
- Manual validation via `cargo test -p xeno-lsp --features position`

## Open Questions (Current State)

1. **Encoding negotiation**: Implemented via `try_capabilities()`. If not initialized, fallback to full sync.
2. **Batching strategy**: Implemented by queueing changes and sending all pending changes per document per tick.
3. **Server capability check**: Implemented in `LspManager::incremental_encoding_for_buffer`.
4. **Multi-cursor ordering**: Implemented by applying transaction ops sequentially against a scratch rope.

## Actionable Next Steps

1. **Cover non-transaction mutations**: Ensure undo/redo or any direct document replacement marks the buffer
   dirty and sends full sync. Suggested targets: `crates/api/src/editor/history.rs`,
   `crates/api/src/editor/lifecycle.rs`.
2. **Add integration tests for LSP batching**: Apply multiple transactions before a tick and verify
   `notify_change_incremental_v2` receives ordered ranges. Suggested targets:
   `crates/api/src/editor/lifecycle.rs`, `crates/lsp/src/sync.rs`.
3. **Add safety fallback thresholds**: If pending change count or total inserted text exceeds a threshold,
   skip incremental and send full sync. Suggested target: `crates/api/src/editor/lifecycle.rs`.
4. **Add telemetry/debug counters**: Track incremental vs full sync sends to validate behavior on large
   files. Suggested targets: `crates/api/src/editor/lifecycle.rs`, `crates/lsp/src/sync.rs`.

## References

- [LSP Spec: textDocument/didChange](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_didChange)
- [Helix implementation](https://github.com/helix-editor/helix/blob/master/helix-lsp/src/lib.rs)
- [VSCode implementation notes](https://github.com/microsoft/vscode/wiki/Incremental-Text-Document-Sync)
