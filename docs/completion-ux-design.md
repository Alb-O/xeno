# Xeno Completion UX Design

Design document from collaborative session with GPT-5.2 Thinking (Jan 2026).

## UX Contract Summary

### When completions appear (auto-popup rules)

1. **Mode gate**: Insert mode only. Normal mode = manual trigger (Ctrl-Space) only.
2. **Event gate**:
   - Typing: may trigger auto-popup
   - Trigger chars (`.`, `::`): immediate popup (0 debounce)
   - Cursor move: NEVER triggers requests (only repositions/closes existing menu)
3. **Prefix gate**: Identifier contexts require prefix >= 1 char. Trigger chars = 0.
4. **Suppression gate**: After Esc dismissal, no auto-popup until:
   - Trigger char typed, OR
   - Manual trigger (Ctrl-Space), OR
   - Leave and re-enter Insert mode

### Selection and Keys

- **No preselect by default**: Menu appears with nothing highlighted
- **Selection intent tracking**: Auto vs Manual (user navigated)
- **Tab**: If selected → accept. Else → select first (no accept on same press)
- **Ctrl-y**: Accept selected, or index 0 if none (explicit commit)
- **Enter**: Always newline (never accepts)
- **Esc**: Dismiss + suppress auto-popup

### Snappiness

- Local filtering on every keystroke (using `filterText` or `label`)
- LSP requests debounced/rate-limited
- Two-tier cache:
  - Tier A: Session cache (raw items for current completion context)
  - Tier B: Buffer-local recent cache (seeds menu instantly on new session)

## Implementation Tasks

### Phase 1: Kill the Annoyances (Easy Wins)

Goal: Immediately improve "feel" without major refactors. Can be done incrementally.

#### 1.1 Remove cursor-move completion triggers
- [ ] In `key_handling.rs`: Remove/gate the `CompletionTrigger::CursorMove` path
- [ ] Keep cursor-move logic for menu repositioning/closing only (no LSP request)
- [ ] File: `crates/api/src/editor/input/key_handling.rs:280-284`

#### 1.2 Fix Enter key behavior
- [ ] In `handle_lsp_menu_key`: Remove `KeyCode::Enter` from accept branch
- [ ] Enter should fall through to normal key handling (newline)
- [ ] File: `crates/api/src/editor/lsp_menu.rs` (find Enter match arm)

#### 1.3 Fix Tab/Ctrl-y semantics
- [ ] Tab: If `selected.is_some()` → accept. Else → select index 0 (no accept)
- [ ] Add Ctrl-y binding: Accept selected, or index 0 if none
- [ ] File: `crates/api/src/editor/lsp_menu.rs` (Tab handling)

#### 1.4 Add suppression/cooldown state
- [ ] Add `suppressed: bool` field to completion state
- [ ] Set `suppressed = true` on Esc dismiss
- [ ] Check suppression in trigger path; block identifier auto-popup
- [ ] Clear suppression on: trigger char, Ctrl-Space, or mode re-entry
- [ ] Files: `crates/api/src/editor/types/completion.rs`, `crates/api/src/editor/lsp_menu.rs`

#### 1.5 Add selection_intent + no preselect
- [ ] Add `SelectionIntent` enum: `Auto | Manual`
- [ ] Add `selection_intent: SelectionIntent` to completion state
- [ ] Default `selected_idx = None` on menu open/refresh (not `Some(0)`)
- [ ] Set `Manual` when user navigates (Down/Up/Ctrl-n/Ctrl-p)
- [ ] File: `crates/api/src/editor/types/completion.rs`, `crates/api/src/editor/lsp_events.rs:89`

---

### Phase 2: Snappy Foundation

Goal: Make completion feel instant. Build filtering/caching infrastructure.

#### 2.1 Create CompletionSession component
- [ ] Create new file: `crates/api/src/editor/completion_session.rs`
- [ ] Define `CompletionSession` struct with fields per Architecture section
- [ ] Define `TriggerKind`, `SelectionIntent`, `ItemKey`, `RawItem` types
- [ ] Define `CompletionCmd` enum for session → editor communication
- [ ] Define `CompletionViewModel` for render path

#### 2.2 Implement session state machine
- [ ] `on_enter_insert()` - clear suppression
- [ ] `on_leave_insert()` - cancel + hide
- [ ] `on_dismiss()` - hide + suppress
- [ ] `on_manual_trigger()` - override suppression, immediate request
- [ ] `on_cursor_move()` - hide if cursor < replace_start (no request)
- [ ] `on_backspace()` - update query, hide if invalid

#### 2.3 Implement typing handlers
- [ ] `on_typed_identifier_char()` - check suppression, seed from Tier B, debounced request
- [ ] `on_typed_trigger_char()` - override suppression, show loading, immediate request
- [ ] `on_typed_coloncolon()` - specialized `::` handling

#### 2.4 Implement local filtering
- [ ] Store `raw_items: Vec<RawItem>` in session
- [ ] Store `filtered_indices: Vec<usize>` computed from query
- [ ] Filter algorithm: prefix match on `filter_text.unwrap_or(label)`
- [ ] Smart-case: if query all-lowercase → case-insensitive; else case-sensitive
- [ ] Preserve server order (no re-ranking)
- [ ] `refilter_active()` called on keystroke and LSP result

#### 2.5 Implement two-tier cache
- [ ] Tier A: `TierACache { raw, filtered }` in `ActiveSession`
- [ ] Tier B: `TierBCacheEntry { raw, timestamp_ms }` per trigger kind
- [ ] `seed_from_tier_b()` - populate menu instantly on session start
- [ ] `update_tier_b()` - store LSP results for future seeding
- [ ] `expire_tier_b()` - TTL ~2500ms, called on gc()

#### 2.6 Implement navigation and accept
- [ ] `on_select_next()` - navigate down, set Manual intent
- [ ] `on_select_prev()` - navigate up, set Manual intent
- [ ] `on_tab()` - select first or accept
- [ ] `on_ctrl_y()` - accept selected or index 0
- [ ] `view_model()` - return `CompletionViewModel`

#### 2.7 Wire CompletionSession into editor
- [ ] Add `CompletionSession` to editor state (next to `CompletionController`)
- [ ] In `key_handling.rs`: call session methods, execute returned `CompletionCmd`
- [ ] In `drain_lsp_ui_events()`: call `session.on_lsp_result()`
- [ ] Update render path to use `session.view_model()`
- [ ] Remove old `CompletionState` usage (or keep as alias to view model)

#### 2.8 Implement loading state UI
- [ ] Add `loading: bool` to view model
- [ ] Render spinner/ellipsis when loading and items empty
- [ ] Show loading only for trigger chars (not identifier typing)

---

### Phase 3: Capability + Stability Upgrades

Goal: Full LSP compliance and rock-solid selection stability.

#### 3.1 Read server trigger characters
- [ ] In `Client`: add method to get `completionProvider.triggerCharacters`
- [ ] In session: only treat char as trigger if server advertises it
- [ ] Fall back to common defaults (`.`, `::`, `->`, `/`) if server doesn't specify

#### 3.2 Selection preservation by identity
- [ ] Implement `ItemKey` computation: hash of `(label, insertText, kind)`
- [ ] Optionally include `data_hash` if LSP item has `data` field
- [ ] On LSP result with Manual intent: find item by key, preserve selection
- [ ] If key not found: fall back to nearest index, not always 0

#### 3.3 Handle isIncomplete
- [ ] In `completion_items_from_response`: extract `is_incomplete` flag
- [ ] Store in session: `is_incomplete: bool`
- [ ] If true: re-request when filtered list becomes empty during typing
- [ ] If false: trust local filtering, don't re-request

#### 3.4 Implement completionItem/resolve
- [ ] Add `resolve_completion_item()` to LSP client
- [ ] Call resolve when selection changes (Manual intent, debounced)
- [ ] Update item's `documentation`, `detail`, `additionalTextEdits`
- [ ] Show resolved docs in completion popup detail area

---

### Implementation Order

Recommended sequence to minimize churn:

```
Phase 1 (do first, in order):
  1.1 → 1.2 → 1.3 → 1.4 → 1.5

Phase 2 (after Phase 1 complete):
  2.1 → 2.2 → 2.3 → 2.6 → 2.7 → 2.4 → 2.5 → 2.8
  
  (Create session shell first, wire it up, then add filtering/caching)

Phase 3 (after Phase 2 stable):
  3.1 → 3.3 → 3.2 → 3.4
  
  (Trigger chars and isIncomplete are quick wins; 
   selection preservation needs care; resolve is optional)
```

### Estimated Effort

| Phase | Tasks | Effort | Impact |
|-------|-------|--------|--------|
| 1 | 5 | 1-2 days | High (fixes annoyances) |
| 2 | 8 | 3-5 days | High (snappy feel) |
| 3 | 4 | 2-3 days | Medium (polish) |

## Architecture: CompletionSession

New component that owns UX state + filtering + caches:

```rust
pub struct CompletionSession {
    active: Option<ActiveSession>,
    suppressed: bool,
    tier_b_identifier: Option<TierBCacheEntry>,
    tier_b_dot: Option<TierBCacheEntry>,
    tier_b_coloncolon: Option<TierBCacheEntry>,
    identifier_min_prefix: usize,  // = 1
    tier_b_ttl_ms: u64,            // ~2500ms
}

struct ActiveSession {
    kind: TriggerKind,
    replace_start: usize,
    query: String,
    loading: bool,
    generation: u64,
    tier_a: TierACache,
    selected: Option<usize>,
    selection_intent: SelectionIntent,
    selected_key: Option<ItemKey>,
}
```

### Key Methods

- `on_enter_insert()` - Clear suppression
- `on_leave_insert()` - Cancel + hide
- `on_dismiss()` - Hide + suppress
- `on_manual_trigger()` - Override suppression, immediate request
- `on_typed_identifier_char()` - Check suppression, seed from Tier B, debounced request
- `on_typed_trigger_char()` - Override suppression, show loading, immediate request
- `on_backspace()` - Update query, hide if invalid
- `on_cursor_move()` - Hide if cursor < replace_start (no request!)
- `on_lsp_result()` - Update Tier A + B, preserve selection if Manual
- `on_select_next/prev()` - Navigate, set Manual intent
- `on_tab()` - Select first or accept
- `on_ctrl_y()` - Accept selected or index 0
- `view_model()` - Return renderable state

### Integration

```
key_handling.rs
    → CompletionSession.on_*()
    → returns CompletionCmd
    → key_handling executes cmd:
        Request → CompletionController.trigger()
        CancelRequest → controller.cancel()
        Accept → apply_completion_item()
        HideMenu → let view_model handle

drain_lsp_ui_events()
    → after generation validation
    → session.on_lsp_result()
    → UI renders from session.view_model()
```

## Acceptance Test Scenarios

### 1. `foo.bar` Happy Path
- Type `f` → menu visible from Tier B cache, no selection
- Type `o`, `o` → instant local filter, debounce restarts
- LSP returns → update in-place, still no selection
- Type `.` → new session, loading state, immediate request
- Type `b`, `a`, `r` → local filter while waiting
- Ctrl-y → accept index 0

### 2. Dismissal Cooldown
- Esc → hide + suppress
- Type `f` → no popup (suppressed)
- Type `.` → popup (trigger char escape hatch)
- Ctrl-Space → popup (manual escape hatch)
- Leave/re-enter Insert → suppression cleared

### 3. Fast Typing
- User types faster than LSP responds
- Menu updates instantly from cache/filter
- Only last debounced request sent
- When response arrives, no selection reset

### 4. Server Latency Spike (500ms+)
- Type `.` → immediate loading state
- Keep typing `bar` → still visible, filtering if cached
- Response arrives → populates already-filtered

### 5. Backspace Past replace_start
- Backspace within token → refilter
- Backspace to empty prefix → hide (identifier min prefix rule)
- Backspace past replace_start → invalidate session

### 6. Mode Switch Mid-Completion
- Esc while menu visible → dismiss + suppress, stay Insert
- Second Esc → Normal mode, suppression irrelevant

## Key Decisions Made

1. **Selection identity key**: `(label, insertText, kind)` tuple, with optional `data_hash`
2. **Filter algorithm**: Prefix match, preserve server order, smart-case
3. **Cache scope**: Tier A = session, Tier B = buffer-local with 2-5s TTL
4. **Loading state**: Show immediately for trigger chars, not for identifier typing
5. **Empty filtered list**: Hide for identifier, stay visible (loading) for trigger chars

## Open Items

- [ ] Time source abstraction for deterministic tests
- [ ] `::` detection location (key handler vs session)
- [ ] Exact grace window for empty filtered lists
- [ ] ItemKey implementation details
