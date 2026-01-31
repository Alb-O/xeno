# Syntax Highlighting Architecture

## Purpose
- Owns: Background parsing scheduling, tiered syntax policy, and grammar loading.
- Does not own: Rendering (owned by buffer render logic), tree-sitter core (external).
- Source of truth: `SyntaxManager`.

## Mental model
- Terms: Tier (S/M/L policy), Hotness (Visible/Warm/Cold), Inflight (background task), Cooldown (backoff after error).
- Lifecycle in one sentence: Edits trigger a debounced background parse, which installs results even if stale to ensure continuous highlighting.

## Module map
- `syntax_manager` — Scheduler, concurrency control, and policy enforcement.
- `runtime::language` — Grammar loading, asset management, and `Syntax` wrapper.

## Key types
| Type | Meaning | Constraints | Constructed / mutated in |
|---|---|---|---|
| `SyntaxManager` | Top-level scheduler | Global concurrency limit | `EditorState` |
| `SyntaxHotness` | Visibility / priority | Affects retention/parsing | Render loop / pipeline |
| `SyntaxTier` | Size-based config (S/M/L) | Controls timeouts/injections | `SyntaxManager::ensure_syntax` |

## Invariants (hard rules)
1. MUST NOT block UI thread on parsing.
   - Enforced in: `SyntaxManager::ensure_syntax` (uses `spawn_blocking`)
   - Tested by: `syntax_manager::tests::test_inflight_drained_even_if_doc_marked_clean`
   - Failure symptom: UI freezes or jitters during edits.
2. MUST enforce single-flight per document.
   - Enforced in: `DocState::inflight` check in `SyntaxManager::ensure_syntax`.
   - Tested by: `scheduler::tests::test_pending_for_doc`
   - Failure symptom: Multiple redundant parse tasks for the same document identity.
3. MUST install last completed parse even if stale, but MUST NOT overwrite a newer clean tree.
   - Enforced in: `should_install_completed_parse` (called from `SyntaxManager::ensure_syntax` poll inflight branch).
   - Tested by: `syntax_manager::tests::test_stale_parse_does_not_overwrite_clean_incremental`, `syntax_manager::tests::test_stale_install_continuity`
   - Failure symptom (missing install): Document stays unhighlighted until an exact match completes.
   - Failure symptom (overwrite race): Stale tree overwrites correct incremental tree while `dirty=false`, creating a stuck state with wrong highlights.
   - Notes: Stale installs are allowed when the caller is already dirty (catch-up mode) or has no syntax tree (bootstrap). A clean tree from a successful incremental update MUST NOT be replaced by an older full-parse result.
4. MUST call `SyntaxManager::note_edit` on every document mutation (edits, undo, redo, LSP workspace edits).
   - Enforced in: `EditorUndoHost::apply_transaction_inner`, `EditorUndoHost::undo_document`, `EditorUndoHost::redo_document`, `Editor::apply_buffer_edit_plan`
   - Tested by: `syntax_manager::tests::test_note_edit_updates_timestamp`
   - Failure symptom: Debounce gate in `SyntaxManager::ensure_syntax` is non-functional; background parses fire without waiting for edit silence.
5. MUST bump `syntax_version` on successful incremental update (commits, undo, redo).
   - Enforced in: `Document::try_incremental_syntax_update`, `Document::incremental_syntax_for_history`
   - Tested by: `buffer::document::tests::test_undo_redo_bumps_syntax_version`
   - Failure symptom: Highlight cache serves stale tiles until background reparse completes, causing a visual lag after undo/redo.

## Data flow
1. Trigger: `SyntaxManager::note_edit` called from edit/undo/redo paths to record debounce timestamp.
2. Render loop: `ensure_syntax_for_buffers` calls `SyntaxManager::ensure_syntax` for each dirty document.
3. Gating: Check visibility, size tier, debounce, and cooldown.
4. Throttling: Acquire global concurrency permit (semaphore).
5. Async boundary: `spawn_blocking` calls `Syntax::new`.
6. Install: Polled result is installed; `dirty` flag cleared only if versions match.

## Lifecycle
- Idle: Document is clean or cooling down.
- Debouncing: Waiting for edit silence.
- In-flight: Background task running.
- Ready: Syntax installed and version matches.

## Concurrency & ordering
- Bounded Concurrency: Max N (default 2) global parse tasks via semaphore.
- Install Discipline: Results only clear `dirty` if `parse_version == current_version`.

## Failure modes & recovery
- Parse Timeout: Set cooldown timer; retry after backoff.
- Grammar Missing: Return `JitDisabled` error; stop retrying for that session.
- Stale Results: Installed to maintain some highlighting, but `dirty` flag triggers eventual catch-up.

## Recipes
### Change tier thresholds
Steps:
- Update `TieredSyntaxPolicy::default()`.
- Ensure `max_bytes_inclusive` logic in `tier_for_bytes` matches.

## Tests
- `buffer::document::tests::commit_syntax_mark_dirty`
- `buffer::document::tests::reset_content_marks_syntax_dirty_and_reparses`
- `scheduler::tests::test_pending_for_doc`
- `syntax_manager::tests::test_note_edit_updates_timestamp`
- `buffer::document::tests::test_undo_redo_bumps_syntax_version`
- `syntax_manager::tests::test_stale_install_continuity`
- `syntax_manager::tests::test_stale_parse_does_not_overwrite_clean_incremental`

## Glossary
- Tier: A set of performance parameters chosen based on document size.
- Hotness: The current visibility state of a document, affecting resource retention.
- Inflight: An active background parsing task.
- Cooldown: A mandatory waiting period after a parsing failure or timeout.
