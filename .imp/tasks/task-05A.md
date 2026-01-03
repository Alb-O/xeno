# Xeno: Read-Only Document Flag - End-to-End Implementation

## Model Directive

Implement a `readonly` boolean flag on `DocumentId` that prevents text modifications. This is a foundational feature enabling permission locking and special-purpose buffers (debug logs, help views, etc.).

**This is an implementation task** - complete the full feature including enforcement, UI feedback, and tests.

______________________________________________________________________

## Implementation Expectations

\<mandatory_execution_requirements>

1. Implement changes incrementally, verifying each step compiles with `cargo check --workspace`
1. Run `cargo test --workspace` after completing the implementation
1. Complete the full implementation - partial solutions are unacceptable
1. If you encounter architectural decisions, document your choice and rationale

Unacceptable:

- Leaving edit paths unguarded
- Breaking existing tests
- Adding the flag without enforcement
- Providing code blocks without writing them to files

\</mandatory_execution_requirements>

______________________________________________________________________

## Behavioral Constraints

\<verbosity_and_scope_constraints>

- Follow existing patterns in the codebase (see Architecture section)
- Prefer minimal changes that achieve correctness
- Do not add features beyond the scope (e.g., no per-selection readonly, no readonly regions)
- If any instruction is ambiguous, choose the simplest valid interpretation
- Do not modify unrelated code paths

\</verbosity_and_scope_constraints>

\<design_freedom>

- You may add helper methods where they improve clarity
- You may refactor edit guard logic if it consolidates enforcement points
- New error variants or notification types are acceptable when needed

\</design_freedom>

______________________________________________________________________

## Design Specification

### Core Requirement

A document-level `readonly: bool` flag that:

1. Defaults to `false` (documents are writable by default)
1. When `true`, blocks ALL text modifications to the document
1. Provides clear user feedback when edit is rejected
1. Is queryable from actions and commands

### Enforcement Points

The readonly flag must be enforced at ALL text modification points:

| Location                                | Method       | Enforcement Strategy      |
| --------------------------------------- | ------------ | ------------------------- |
| `Buffer::apply_transaction`             | Direct edit  | Return early, notify user |
| `Buffer::apply_transaction_with_syntax` | Direct edit  | Return early, notify user |
| `Editor::do_execute_edit_action`        | Edit actions | Guard at top, notify user |
| `Editor::insert_text`                   | Insert mode  | Guard before transaction  |
| Insert mode character handling          | Key events   | Guard before insert       |

### API Design

```rust
// In Document (crates/api/src/buffer/document.rs)
pub struct Document {
    // ... existing fields ...
    /// Whether the document is read-only (prevents all text modifications).
    pub readonly: bool,
}

// Accessor on Buffer (crates/api/src/buffer/mod.rs)
impl Buffer {
    /// Returns whether the underlying document is read-only.
    pub fn is_readonly(&self) -> bool {
        self.document.read().unwrap().readonly
    }

    /// Sets the read-only flag on the underlying document.
    pub fn set_readonly(&self, readonly: bool) {
        self.document.write().unwrap().readonly = readonly;
    }
}
```

### User Feedback

When an edit is rejected due to readonly:

1. Display notification: `"Buffer is read-only"` (type: `"warning"`)
1. Do NOT change mode (if in insert mode, stay in insert mode - the user may want to navigate)
1. Do NOT beep or produce other disruptive feedback

### Command Interface

Add a `:readonly` command (with alias `:ro`) that toggles the readonly state:

```rust
command!(readonly, {
    description: "Toggle read-only mode for current buffer",
    aliases: ["ro"],
}, |ctx| {
    let current = ctx.buffer().is_readonly();
    ctx.buffer_mut().set_readonly(!current);
    let msg = if !current { "Read-only enabled" } else { "Read-only disabled" };
    ctx.notify("info", msg);
    Ok(())
});
```

______________________________________________________________________

## Implementation Roadmap

### Phase 1: Add the Flag

**Files:** `crates/api/src/buffer/document.rs`, `crates/api/src/buffer/mod.rs`

Tasks:

1. Add `pub readonly: bool` field to `Document` struct, initialize to `false` in constructors
1. Add `is_readonly()` and `set_readonly()` methods to `Buffer`
1. Verify: `cargo check -p xeno-api`

### Phase 2: Enforce at Transaction Level

**Files:** `crates/api/src/buffer/editing.rs`

Tasks:

1. Modify `apply_transaction` to return `bool` (true = applied, false = rejected)
1. Add readonly check at start: if readonly, return false without modifying content
1. Modify `apply_transaction_with_syntax` similarly
1. Update all callers to handle the return value (most can ignore it for now)
1. Verify: `cargo check --workspace`

### Phase 3: Guard Edit Actions

**Files:** `crates/api/src/editor/actions.rs`

Tasks:

1. Add readonly guard at the top of `do_execute_edit_action`:
   ```rust
   if self.buffer().is_readonly() {
       self.notify("warning", "Buffer is read-only");
       return;
   }
   ```
1. Verify: `cargo check -p xeno-api`

### Phase 4: Guard Insert Mode

**Files:** `crates/api/src/editor/input/mod.rs` (or wherever insert mode key handling lives)

Tasks:

1. Find insert mode character handling
1. Add readonly guard before text insertion
1. Consider: should entering insert mode be blocked on readonly buffers? (Recommendation: NO - allow navigation, just block edits)
1. Verify: `cargo check --workspace`

### Phase 5: Add Command

**Files:** `crates/registry/commands/src/impls/` (new file or existing buffer commands file)

Tasks:

1. Add `readonly` command using the `command!` macro
1. Implement toggle logic with notification feedback
1. Verify: `cargo check -p xeno-registry-commands`

### Phase 6: Add Statusline Indicator (Optional but Recommended)

**Files:** `crates/registry/statusline/src/impls/`

Tasks:

1. Add statusline segment showing `[RO]` when buffer is readonly
1. Use existing statusline segment pattern
1. Verify: `cargo check -p xeno-registry-statusline`

### Phase 7: Testing

**Files:** `crates/api/src/buffer/` (unit tests), `crates/term/tests/` (integration tests if needed)

Tasks:

1. Add unit tests for readonly enforcement:
   - Test that `apply_transaction` returns false when readonly
   - Test that `is_readonly()` / `set_readonly()` work correctly
1. Verify: `cargo test --workspace`

### Phase 8: Final Verification

Tasks:

1. Run `cargo clippy --workspace`
1. Verify all edit paths are guarded (grep for `apply_transaction`, `insert_text`, etc.)
1. Manual smoke test if possible

______________________________________________________________________

## Key Files Reference

```
crates/api/src/buffer/document.rs     # Document struct - ADD readonly field
crates/api/src/buffer/mod.rs          # Buffer struct - ADD accessor methods
crates/api/src/buffer/editing.rs      # apply_transaction - ADD enforcement
crates/api/src/editor/actions.rs      # do_execute_edit_action - ADD guard
crates/api/src/editor/input/          # Insert mode handling - ADD guard
crates/registry/commands/src/         # Commands - ADD :readonly command
crates/registry/statusline/src/       # Statusline - ADD [RO] indicator
```

______________________________________________________________________

## Edge Cases

1. **Undo/Redo on readonly buffer**: Should be BLOCKED - undo/redo are modifications
1. **Paste on readonly buffer**: Should be BLOCKED
1. **Replace char on readonly buffer**: Should be BLOCKED
1. **Indent/Deindent on readonly buffer**: Should be BLOCKED
1. **Split views of readonly document**: All views see the same readonly state (it's on Document, not Buffer)
1. **Setting readonly while in insert mode**: Should be allowed; subsequent edits blocked
1. **File save on readonly buffer**: Should be ALLOWED - readonly is editor-level, not filesystem-level

______________________________________________________________________

## Anti-Patterns to Avoid

1. **Don't check readonly at every individual action**: Centralize enforcement in `apply_transaction` and `do_execute_edit_action`
1. **Don't add readonly to Buffer**: It belongs on Document since it's shared across split views
1. **Don't silently fail**: Always notify the user when an edit is rejected
1. **Don't block mode changes**: User should be able to enter visual mode, search, etc. on readonly buffers
1. **Don't conflate with file permissions**: This is an editor-level lock, independent of filesystem readonly

______________________________________________________________________

## Success Criteria

- [ ] `Document` has `readonly: bool` field, defaulting to `false`
- [ ] `Buffer::is_readonly()` and `Buffer::set_readonly()` methods exist
- [ ] `apply_transaction` and `apply_transaction_with_syntax` enforce readonly
- [ ] `do_execute_edit_action` guards against readonly with notification
- [ ] Insert mode text insertion is blocked on readonly buffers
- [ ] `:readonly` command toggles the flag with feedback
- [ ] All modifications blocked: delete, change, paste, undo, redo, indent, etc.
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` clean (or no new warnings)

______________________________________________________________________

## Architecture Context

### Document vs Buffer

- `Document` holds shared content (text, undo history, syntax) - owned by `Arc<RwLock<Document>>`
- `Buffer` holds per-view state (cursor, selection, scroll) - wraps the Document
- Multiple Buffers can share one Document (split views)
- **The readonly flag belongs on Document** so all views of the same file share the lock

### Edit Flow

```
User action → Action handler → EditAction variant
    → do_execute_edit_action() → Transaction::* → apply_transaction()
```

For insert mode:

```
Key event → Input handler → character insertion → insert_text() → apply_transaction()
```

### Capability System

The editor uses fine-grained capability traits (see `crates/registry/actions/src/editor_ctx/capabilities.rs`):

- `EditAccess::execute_edit()` - main edit entry point from actions
- Actions call `ctx.edit()?.execute_edit(&action, extend)`

The readonly check should happen in the concrete implementation (`Editor::do_execute_edit_action`), not in the trait.
