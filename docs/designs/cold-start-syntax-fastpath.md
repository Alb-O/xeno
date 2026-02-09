# Cold-Start Syntax Highlight Fast Path

## Problem

When a file is first opened, Xeno always delegates parsing to a background task
(`spawn_blocking`). The result is installed on the next `tick()` → `render()`
cycle. This means highlights never appear on the first rendered frame:

```
Frame 1: render() → ensure_syntax() → spawn_blocking(parse) → no highlights
Frame 2: tick()   → drain_finished  → install tree → request redraw
Frame 3: render() → highlights visible
```

There is always a minimum 1–2 frame gap (~16–32 ms at 60 fps) before any
highlighting appears. For files where tree-sitter parsing completes in <5 ms
(most files under ~50 KB), this delay is pure overhead — the parse would have
finished well within the frame budget.

Neovim avoids this by parsing synchronously first and only falling back to
async when the parse exceeds a per-frame budget. If the parse completes, the
first frame is already highlighted.

## Design

Add a synchronous parse attempt to the bootstrap path in `ensure_syntax`. When
a document has no existing syntax tree (first open), try parsing inline with a
tight timeout before falling back to `spawn_blocking`.

### Key constraint

`ensure_syntax` runs inside `render()` on the UI thread. The sync attempt must
be bounded to a wall-clock budget that does not cause a visible frame stutter.
A 5 ms budget is safe for 60 fps (16.6 ms frame budget) and covers most
small-to-medium files.

### Activation criteria

The sync fast path fires when ALL of these hold:

- **Bootstrap**: `entry.slot.current` is `None` (no existing tree).
- **Visible**: `ctx.hotness` is `Visible` (actively displayed in a window).
- **Has language**: `ctx.language_id` is `Some`.
- **No active task**: `entry.sched.active_task` is `None`.
- **Tier allows it**: controlled by a new `TierCfg::sync_bootstrap_timeout`
  field. S-tier gets `Some(Duration::from_millis(5))`, M-tier gets
  `Some(Duration::from_millis(3))`, L-tier gets `None` (never attempt sync —
  a 4 MB file will not parse in 5 ms).

When any condition is false, fall through to the existing `spawn_blocking`
path unchanged.

### Timeout behavior

tree-sitter's `set_timeout` API (currently used by tree-house) returns `None`
on timeout — no partial tree, no preserved progress. A failed sync attempt is
wasted work. This is acceptable because:

1. The timeout is small (3–5 ms), so wasted work is small.
2. The background task will immediately take over and parse with the full tier
   timeout (500 ms for S, 1200 ms for M).
3. The fast path fires only once per document (bootstrap). Subsequent edits
   use the existing sync incremental path (`note_edit_incremental`).

A future improvement (item #3 in the comparison: `ts_parser_parse_with_options`
progress callback) would let the sync attempt preserve parser state for the
background task to resume, eliminating the wasted work. This design is
compatible with that extension.

### SyntaxEngine interaction

The sync fast path calls `engine.parse()` directly on the render thread — the
same `SyntaxEngine` method used by background tasks. No new parsing API is
needed. The only difference is the `SyntaxOptions::parse_timeout` value.

For testability, the `SyntaxEngine` trait is already injected into
`SyntaxManager`. Mock engines in tests can simulate fast or slow parses to
exercise both the sync-success and sync-timeout-then-background paths.

## Changes

### `TierCfg`

```rust
pub struct TierCfg {
    // ... existing fields ...
    /// Timeout for the synchronous bootstrap parse attempt on the render
    /// thread. `None` disables the fast path for this tier.
    pub sync_bootstrap_timeout: Option<Duration>,
}
```

Defaults:

| Tier | `sync_bootstrap_timeout` | Rationale |
|------|--------------------------|-----------|
| S    | `Some(5ms)`              | Most files under 256 KB parse in <5 ms |
| M    | `Some(3ms)`              | Tighter budget, lower success rate, still worth trying |
| L    | `None`                   | Files over 1 MB will not parse in any useful sync budget |

### `SyntaxManager::ensure_syntax`

Insert a new step between the current "gating" checks and "schedule new task",
roughly at the point where the code currently falls through to step 6
("Schedule new task"). Pseudocode:

```rust
// --- NEW: Step 5.5 — Sync bootstrap fast path ---
let is_bootstrap = entry.slot.current.is_none();
let is_visible = matches!(ctx.hotness, SyntaxHotness::Visible);

if is_bootstrap && is_visible {
    if let Some(sync_timeout) = cfg.sync_bootstrap_timeout {
        let sync_opts = SyntaxOptions {
            parse_timeout: sync_timeout,
            injections: cfg.injections,
            build_locals: cfg.build_locals,
        };
        match self.engine.parse(
            ctx.content.slice(..),
            lang_id,
            ctx.loader,
            sync_opts,
        ) {
            Ok(syntax) => {
                entry.slot.current = Some(syntax);
                entry.slot.language_id = Some(lang_id);
                entry.slot.tree_doc_version = Some(ctx.doc_version);
                entry.slot.dirty = false;
                entry.slot.pending_incremental = None;
                mark_updated(&mut entry.slot);
                return SyntaxPollOutcome {
                    result: SyntaxPollResult::Ready,
                    updated: true,
                };
            }
            Err(_) => {
                // Sync attempt timed out or failed.
                // Fall through to spawn_blocking as usual.
            }
        }
    }
}
// --- END sync fast path ---

// 6. Schedule new task (existing code, unchanged)
```

### `SyntaxEngine` trait

No changes needed. `parse()` is already `&self` (shared reference) and
stateless. Calling it on the render thread is safe. The `Arc<dyn SyntaxEngine>`
is already available in `SyntaxManager`.

However, `engine.parse()` currently takes `ropey::RopeSlice`, `LanguageId`,
`&LanguageLoader`, and `SyntaxOptions` — all of which are available from
`EnsureSyntaxContext` and the computed tier config. No new parameters are
needed.

### `SyntaxPollResult`

No new variants needed. On sync success, return `SyntaxPollResult::Ready`
(tree is installed). On sync failure, fall through to `Kicked` / `Throttled`
as before.

### Invariants

New invariant for `invariants.rs`:

> Must attempt sync bootstrap only when `slot.current` is `None`,
> `hotness` is `Visible`, and `sync_bootstrap_timeout` is `Some`.

> Must fall through to the background path on sync failure without
> entering a cooldown or error state.

## Frame timing analysis

For a typical small Rust file (~5 KB):

```
Before:
  Frame 1: ensure_syntax → spawn_blocking(parse ~0.5ms) → no highlights
  Frame 2: tick → drain → install → redraw
  Frame 3: render → highlights visible
  Total: ~32 ms (2 frames at 60 fps)

After:
  Frame 1: ensure_syntax → sync parse (0.5ms) → installed → highlights visible
  Total: ~0.5 ms
```

For a medium file (~200 KB) that parses in ~4 ms:

```
After:
  Frame 1: ensure_syntax → sync parse (4ms) → installed → highlights visible
  Total: ~4 ms (within the 5ms S-tier budget)
```

For a medium file (~500 KB) that parses in ~8 ms (exceeds sync budget):

```
After:
  Frame 1: ensure_syntax → sync parse timeout (5ms wasted) → spawn_blocking
  Frame 2: tick → drain → install → redraw
  Frame 3: render → highlights visible
  Total: ~37 ms (5ms wasted + 2 frames)
```

The 5 ms penalty on timeout is the cost of this optimization. It's bounded,
small, and only happens once per document. For the common case (small files),
it eliminates a visible flash of unhighlighted text.

## Files modified

| File | Change |
|------|--------|
| `crates/editor/src/syntax_manager/mod.rs` | Add `sync_bootstrap_timeout` to `TierCfg`, sync attempt in `ensure_syntax` |
| `crates/editor/src/syntax_manager/invariants.rs` | New invariant test for sync bootstrap gating |

## Testing

1. Unit test with a mock `SyntaxEngine` that returns `Ok` in <1 ms: verify
   `ensure_syntax` returns `Ready` on the first call (no background task
   spawned).

2. Unit test with a mock engine that returns `Err(Timeout)`: verify
   `ensure_syntax` falls through to `Kicked` (background task spawned), and
   no cooldown is set.

3. Unit test with `hotness = Cold`: verify sync path is not attempted even on
   bootstrap.

4. Unit test with L-tier file: verify sync path is not attempted
   (`sync_bootstrap_timeout` is `None`).

5. Manual test: open a small Rust file, observe highlights on the first visible
   frame (no flash of plain text).
