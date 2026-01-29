# Syntax Highlighting Architecture

## Purpose
- Owns: Background parsing scheduling, tiered syntax policy, and grammar loading.
- Does not own: Rendering (owned by buffer render logic), tree-sitter core (external).
- Source of truth: `SyntaxManager` in `crates/editor/src/syntax_manager.rs`.

## Mental model
- Terms: Tier (S/M/L policy), Hotness (Visible/Warm/Cold), Inflight (background task), Cooldown (backoff after error).
- Lifecycle in one sentence: Edits trigger a debounced background parse, which installs results even if stale to ensure continuous highlighting.

## Module map
- `crates/editor/src/syntax_manager.rs` — Scheduler, concurrency control, and policy enforcement.
- `crates/runtime/language/` — Grammar loading, asset management, and `Syntax` wrapper.

## Key types
| Type | Meaning | Constraints | Constructed / mutated in |
|---|---|---|---|
| `SyntaxManager` | Top-level scheduler | Global concurrency limit | `EditorState` |
| `SyntaxHotness` | Visibility / priority | Affects retention/parsing | Render loop / pipeline |
| `SyntaxTier` | Size-based config (S/M/L) | Controls timeouts/injections | `crates/editor/src/syntax_manager.rs`::`SyntaxManager::ensure_syntax` |

## Invariants (hard rules)
1. MUST NOT block UI thread on parsing.
   - Enforced in: `crates/editor/src/syntax_manager.rs`::`SyntaxManager::ensure_syntax` (uses `spawn_blocking`)
   - Tested by: TODO (add regression: test_async_parsing)
   - Failure symptom: UI freezes or jitters during edits.
2. MUST enforce single-flight per document.
   - Enforced in: `DocState::inflight` check in `crates/editor/src/syntax_manager.rs`::`SyntaxManager::ensure_syntax`.
   - Tested by: `crates/editor/src/syntax_manager.rs`::`scheduler::tests::test_pending_for_doc`
   - Failure symptom: Multiple redundant parse tasks for the same document identity.
3. MUST install last completed parse even if stale.
   - Enforced in: `crates/editor/src/syntax_manager.rs`::`SyntaxManager::ensure_syntax` (poll inflight branch).
   - Tested by: TODO (add regression: test_stale_install_continuity)
   - Failure symptom: Document stays unhighlighted until an exact match completes.
   - Notes: Stale installs improve continuity, but `dirty` remains to force catch-up.

## Data flow
1. Trigger: `note_edit` or `ensure_syntax` called from render loop.
2. Gating: Check visibility, size tier, debounce, and cooldown.
3. Throttling: Acquire global concurrency permit (semaphore).
4. Async boundary: `spawn_blocking` calls `Syntax::new`.
5. Install: Polled result is installed; `dirty` flag cleared only if versions match.

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
- Update `TieredSyntaxPolicy::default()` in `crates/editor/src/syntax_manager.rs`.
- Ensure `max_bytes_inclusive` logic in `tier_for_bytes` matches.

## Tests
- `crates/editor/src/buffer/document/tests.rs`::`commit_syntax_mark_dirty`
- `crates/editor/src/buffer/document/tests.rs`::`reset_content_marks_syntax_dirty_and_reparses`
- `crates/editor/src/syntax_manager.rs`::`scheduler::tests::test_pending_for_doc`

## Glossary
- Tier: A set of performance parameters chosen based on document size.
- Hotness: The current visibility state of a document, affecting resource retention.
- Inflight: An active background parsing task.
- Cooldown: A mandatory waiting period after a parsing failure or timeout.
