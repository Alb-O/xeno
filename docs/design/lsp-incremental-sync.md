# LSP Incremental Document Sync

**Status**: Proposed  
**Author**: Design session  
**Date**: 2026-01-08

## Problem Statement

LSP's `textDocument/didChange` notification supports two sync modes:

1. **Full sync**: Send entire document content on every change
2. **Incremental sync**: Send only the changed range + replacement text

We currently use full sync exclusively. The incremental sync API exists but is broken: it receives post-change text but needs to compute positions in pre-change text.

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

## Current Architecture

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
│                  mark buffer dirty                           │
│                         │                                    │
│                         ▼                                    │
│               ┌─────────────────┐                           │
│               │  tick() loop    │                           │
│               │  - emit hook    │                           │
│               │  - LSP sync     │◀── POST-change text only  │
│               └─────────────────┘                           │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
              ┌───────────────────────┐
              │   DocumentSync        │
              │   .notify_change_full │  ◀── Works correctly
              │   .notify_change_incr │  ◀── BROKEN: wrong positions
              └───────────────────────┘
```

### The Bug

```rust
// sync.rs - current broken implementation
pub async fn notify_change_incremental(
    &self,
    path: &Path,
    language: &str,
    text: &Rope,           // ← POST-change text
    start_char: usize,     // ← Should be PRE-change position
    end_char: usize,       // ← Should be PRE-change position
    new_text: &str,
    encoding: OffsetEncoding,
) -> Result<()> {
    // BUG: Computing LSP Range from POST-change text
    // but positions reference PRE-change state
    let range = char_range_to_lsp_range(text, start_char, end_char, encoding);
    //                                  ^^^^ WRONG TEXT
}
```

If you insert a line at line 5, then try to report a change at "old line 10", computing the position from the new text gives you line 11.

## Proposed Solution

### Core Insight

LSP positions must be computed **before** the transaction is applied. This means the edit layer needs to emit change events with pre-computed positions.

### Architecture Change

```
┌─────────────────────────────────────────────────────────────┐
│                        Editor                                │
│                                                              │
│  ┌──────────┐    ┌─────────────┐    ┌──────────────────┐   │
│  │  Buffer  │───▶│ Transaction │───▶│ Document (Rope)  │   │
│  │  .edit() │    │             │    │                  │   │
│  └──────────┘    │  BEFORE:    │    └──────────────────┘   │
│                  │  - compute  │                            │
│                  │    LSP range│                            │
│                  │  - queue    │                            │
│                  │    change   │                            │
│                  │             │                            │
│                  │  AFTER:     │                            │
│                  │  - apply to │                            │
│                  │    rope     │                            │
│                  └─────────────┘                            │
│                         │                                    │
│                         ▼                                    │
│               ┌─────────────────┐                           │
│               │  Change Queue   │◀── Pre-computed LSP data  │
│               │  Vec<LspChange> │                           │
│               └─────────────────┘                           │
│                         │                                    │
│                         ▼                                    │
│               ┌─────────────────┐                           │
│               │  tick() loop    │                           │
│               │  - drain queue  │                           │
│               │  - send to LSP  │                           │
│               └─────────────────┘                           │
└─────────────────────────────────────────────────────────────┘
```

### New Types

```rust
// In xeno-base or xeno-lsp

/// A pre-computed LSP document change, ready to send.
#[derive(Debug, Clone)]
pub struct LspDocumentChange {
    /// The range in the document that was replaced (pre-change positions).
    pub range: lsp_types::Range,
    /// The text that replaced the range.
    pub new_text: String,
}

/// Accumulated changes for a single document version bump.
#[derive(Debug, Clone)]
pub struct LspChangeSet {
    /// Path to the document.
    pub path: PathBuf,
    /// Language ID.
    pub language: String,
    /// Individual changes (may be multiple for a single transaction).
    pub changes: Vec<LspDocumentChange>,
}
```

### Integration Points

#### Option A: Transaction-level integration

Add LSP position computation directly to `Transaction`:

```rust
// xeno-base/src/transaction.rs

impl Transaction {
    /// Apply transaction and return LSP change data.
    pub fn apply_with_lsp(
        self,
        rope: &mut Rope,
        encoding: OffsetEncoding,
    ) -> Vec<LspDocumentChange> {
        let mut lsp_changes = Vec::new();
        
        for op in &self.operations {
            match op {
                Operation::Insert { pos, text } => {
                    // Compute LSP position BEFORE modifying rope
                    let lsp_pos = char_to_lsp_position(rope, *pos, encoding);
                    let range = Range { start: lsp_pos, end: lsp_pos };
                    
                    lsp_changes.push(LspDocumentChange {
                        range,
                        new_text: text.clone(),
                    });
                    
                    // Now apply
                    rope.insert(*pos, text);
                }
                Operation::Delete { start, end } => {
                    // Compute LSP range BEFORE modifying rope
                    let range = char_range_to_lsp_range(rope, *start, *end, encoding);
                    
                    lsp_changes.push(LspDocumentChange {
                        range,
                        new_text: String::new(),
                    });
                    
                    // Now apply
                    rope.remove(*start..*end);
                }
                // ... etc
            }
        }
        
        lsp_changes
    }
}
```

**Pros**: Clean, single source of truth  
**Cons**: Adds LSP dependency to xeno-base (or needs abstraction)

#### Option B: Buffer-level wrapper

Keep Transaction LSP-agnostic, compute positions in Buffer:

```rust
// xeno-api/src/buffer.rs

impl Buffer {
    pub fn apply_edit_with_lsp(&mut self, tx: Transaction) -> Vec<LspDocumentChange> {
        let encoding = self.lsp_encoding.unwrap_or_default();
        let rope = &self.doc().content;
        
        // Compute positions before apply
        let lsp_changes = tx.operations.iter().map(|op| {
            compute_lsp_change(rope, op, encoding)
        }).collect();
        
        // Apply transaction
        self.apply_transaction(tx);
        
        lsp_changes
    }
}
```

**Pros**: No LSP in base crate, cleaner separation  
**Cons**: Logic split across crates, potential for drift

#### Option C: Event/Observer pattern

Transaction emits events, LSP layer observes:

```rust
// In Transaction
pub trait TransactionObserver {
    fn before_insert(&mut self, rope: &Rope, pos: usize, text: &str);
    fn before_delete(&mut self, rope: &Rope, start: usize, end: usize);
}

impl Transaction {
    pub fn apply_observed<O: TransactionObserver>(
        self, 
        rope: &mut Rope,
        observer: &mut O,
    ) {
        for op in self.operations {
            match op {
                Operation::Insert { pos, text } => {
                    observer.before_insert(rope, pos, &text);
                    rope.insert(pos, &text);
                }
                // ...
            }
        }
    }
}
```

**Pros**: Flexible, decoupled  
**Cons**: More complex, observer lifetime management

### Recommended Approach: Option B

Buffer-level wrapper is the sweet spot:

1. **No new dependencies** in xeno-base
2. **Single integration point** in Buffer
3. **Easy to test** - Buffer already has tests
4. **Backwards compatible** - existing code keeps working

### Implementation Plan

#### Phase 1: Infrastructure (xeno-base)

1. Add `LspDocumentChange` type to xeno-base (no lsp_types dep, use own Range type)
2. Add helper to iterate Transaction operations without consuming
3. Ensure Transaction operations are inspectable

#### Phase 2: Position computation (xeno-lsp)

1. Add `compute_lsp_changes(rope: &Rope, tx: &Transaction, encoding) -> Vec<LspDocumentChange>`
2. Unit tests with various edit scenarios (insert, delete, replace, multi-cursor)

#### Phase 3: Buffer integration (xeno-api)

1. Add `Buffer::apply_edit_with_lsp()` method
2. Store pending LSP changes in Buffer or Frame
3. Update edit methods to use new path when LSP is enabled

#### Phase 4: Sync layer (xeno-lsp)

1. Update `DocumentSync` to accept pre-computed changes
2. Add `notify_change_incremental_v2(changes: Vec<LspDocumentChange>)`
3. Deprecation path for old API

#### Phase 5: Wire up (xeno-api)

1. Update `tick()` to drain LSP changes and send
2. Handle batching (multiple edits between ticks)
3. Remove old full-sync path (or keep as fallback)

### Testing Strategy

```rust
#[test]
fn test_insert_computes_correct_range() {
    let mut rope = Rope::from("hello\nworld\n");
    let tx = Transaction::insert(6, "beautiful ");
    
    let changes = compute_lsp_changes(&rope, &tx, OffsetEncoding::Utf16);
    
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].range.start, Position { line: 1, character: 0 });
    assert_eq!(changes[0].range.end, Position { line: 1, character: 0 });
    assert_eq!(changes[0].new_text, "beautiful ");
}

#[test]
fn test_delete_line_computes_correct_range() {
    let mut rope = Rope::from("line1\nline2\nline3\n");
    let tx = Transaction::delete(6, 12); // delete "line2\n"
    
    let changes = compute_lsp_changes(&rope, &tx, OffsetEncoding::Utf16);
    
    assert_eq!(changes[0].range.start, Position { line: 1, character: 0 });
    assert_eq!(changes[0].range.end, Position { line: 2, character: 0 });
    assert_eq!(changes[0].new_text, "");
}

#[test]  
fn test_multi_cursor_edit() {
    // Multiple insertions in single transaction
    // Positions must be computed sequentially as each edit shifts subsequent positions
}
```

### Migration Path

1. **v0.3**: Deprecate `notify_change_incremental` (done)
2. **v0.4**: Add new incremental sync, keep full sync as default
3. **v0.5**: Switch default to incremental, full sync opt-in
4. **v1.0**: Remove full sync fallback (or keep for compatibility)

### Open Questions

1. **Encoding negotiation**: When do we know the server's preferred encoding? After `initialize` response. Need to handle the window between open and initialize.

2. **Batching strategy**: If user types fast, do we send one change per keystroke or batch? LSP supports multiple changes per notification.

3. **Server capability check**: Should we fall back to full sync if server doesn't advertise incremental support?

4. **Multi-cursor ordering**: When applying multiple cursors, positions shift. Need to process in reverse document order or track cumulative offset.

### References

- [LSP Spec: textDocument/didChange](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_didChange)
- [Helix implementation](https://github.com/helix-editor/helix/blob/master/helix-lsp/src/lib.rs)
- [VSCode implementation notes](https://github.com/microsoft/vscode/wiki/Incremental-Text-Document-Sync)
