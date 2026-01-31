# Windowing and Split Layout System

## Purpose
- Owns: window containers (base + floating), stacked layout layers, split-tree geometry (`buffer::Layout`), view navigation, separator hit/resize/drag state, and editor-level split/close integration.
- Does not own: buffer/document content (owned by buffer/document subsystems), UI widget styling (owned by renderer + widget layer), overlay session policy (owned by overlay system; windowing only hosts floating windows and overlay layouts).
- Source of truth:
  - Layout/layers/splits/navigation: `LayoutManager` + `buffer::Layout`
  - Base/floating window containers: `WindowManager`, `BaseWindow`, `FloatingWindow`
  - Editor integration: `Editor` split/close methods

## Mental model
- There is exactly one base split tree (owned by `BaseWindow.layout`).
- There are zero or more overlay split trees (owned by `LayoutManager.layers: Vec<LayerSlot>`).
- Every leaf in any split tree is a `ViewId`.
- Any reference to an overlay layer that can outlive the immediate call stack MUST use `LayerId { idx, generation }` and MUST be validated before use.
- A separator is identified by `(LayerId, SplitPath)`; resizing moves `Layout::Split.position` at that path.
- Split operations are atomic at editor level: feasibility is checked before buffer allocation and before mutating layout.

## Module map
- `window::types` — `WindowId`, `Window`, `BaseWindow`, `FloatingWindow`, `FloatingStyle`, `GutterSelector`.
- `window::manager` — `WindowManager`: owns base window + floating windows + z-order.
- `window::floating` — `FloatingWindow` helpers (`contains`, `content_rect`).
- `layout::types` — `LayerId`, `LayerSlot`, `LayerError`, `SeparatorId`, `SeparatorHit`.
- `layout::manager` — `LayoutManager` core state: `layers`, `layout_revision`, hover/drag state.
- `layout::layers` — `validate_layer`, `layer/layer_mut`, `set_layer`, `top_layer`, `layer_of_view`, `overlay_layout` helpers.
- `layout::splits` — Split preflight (`can_split_*`), split apply, view removal (`remove_view`) + focus suggestion.
- `layout::views` — View enumeration/navigation/hit-testing across layers; uses `overlay_layout()` for overlay access.
- `layout::separators` — Separator hit-testing across layers, rect lookup, resize-by-id.
- `layout::drag` — Drag lifecycle + stale detection (revision + generation).
- `buffer::layout` — `Layout` binary tree, min sizes, replace/remove primitives.
- `buffer::layout::areas` — `compute_split_areas` soft-min policy, separator paths, resizing.
- `buffer::layout::navigation` — `next/prev` ordering and spatial direction navigation.
- `buffer::layout::tests` — Invariant + regression tests for split geometry and navigation.
- `layout` — Basic `LayoutManager` tests (layer area, hit-testing).
- `impls::splits` — Editor entrypoints: split with clone, split apply, close view/buffer, hook ordering.
- `registry::actions::builtins::window` — User-facing actions mapped to effects (split, focus, close).

## Key types
| Type | Meaning | Constraints | Constructed / mutated in |
|---|---|---|---|
| `WindowManager` | Owns all windows (base + floating) | MUST always contain exactly one base window | `WindowManager::new`, `WindowManager::create_floating`, `WindowManager::close_floating` |
| `BaseWindow` | Base split tree container | `layout` is the base layer tree; `focused_buffer` MUST be a view in the base tree after repairs | editor state |
| `FloatingWindow` | Absolute-positioned overlay window | Pure data; policy fields are enforced elsewhere | `FloatingWindow::new` |
| `LayoutManager` | Owns overlay layers + separator interaction | `layers[0]` is a dummy slot; base layout lives outside slots | `LayoutManager` + `layout::*` methods |
| `LayerId` | Generational layer handle | MUST validate before deref unless `is_base()` | `layout::types` + `layout::layers` |
| `LayerSlot` | Storage slot for overlay layer | `generation` MUST bump when layer identity ends (clear/replace) | `LayoutManager::set_layer`, `LayoutManager::remove_view` |
| `LayerError` | Layer validation failure | MUST treat as stale/invalid and no-op/cancel | `LayoutManager::validate_layer` |
| `Layout` | Split tree for arranging `ViewId` leaves | `position` is parent-local; geometry MUST obey soft-min policy | `buffer::layout` + `buffer::layout::areas` |
| `SplitPath(Vec<bool>)` | Stable path to a split node | Path is relative to the current tree shape; stale paths MUST be rejected | `buffer::layout::areas` path APIs |
| `SeparatorId::Split{layer,path}` | Persistent separator identity | MUST validate layer generation + path before resize | `layout::separators` + `layout::drag` |
| `SplitError` | Split preflight failure | MUST not allocate/insert buffers when preflight fails | `layout::splits` + `impls::splits` |
| `DragState` | Active separator drag | MUST cancel when revision or layer generation invalidates id | `layout::drag` |
| `ViewId` | Leaf identity in layouts | A `ViewId` MUST not exist in multiple layers simultaneously | enforced by editor invariants/repair |

## Invariants (hard rules)
1. MUST validate any stored `LayerId` before accessing an overlay layout.
   - Enforced in: `LayoutManager::validate_layer`, `LayoutManager::overlay_layout`, `LayoutManager::overlay_layout_mut`, `LayoutManager::layer`, `LayoutManager::layer_mut`
   - Tested by: TODO (add regression: test_layerid_generation_rejects_stale)
   - Failure symptom: separator drag/resize or focus targets operate on the wrong overlay after a layer is cleared/reused.

2. MUST preserve `LayerId` generation across split preflight → apply.
   - Enforced in: `LayoutManager::can_split_horizontal`, `LayoutManager::can_split_vertical` and split apply APIs taking `LayerId`
   - Tested by: TODO (add regression: test_split_preflight_apply_generation_preserved)
   - Failure symptom: split applies to the wrong layer if the overlay slot is replaced between check and apply.

3. MUST NOT allocate/insert a new `ViewId` for a split if the split cannot be created.
   - Enforced in: `Editor::split_horizontal_with_clone`, `Editor::split_vertical_with_clone`, `Editor::split_horizontal`, `Editor::split_vertical` (preflight before buffer creation) and `SplitError`
   - Tested by: TODO (add regression: test_split_preflight_no_orphan_buffer)
   - Failure symptom: orphan `ViewId` exists in buffer store but not in any layout; focus may jump to a non-rendered view.

4. MUST emit close hooks only after the view has been removed from layout successfully.
   - Enforced in: `Editor::close_view`
   - Tested by: TODO (add regression: test_close_view_hooks_after_removal)
   - Failure symptom: hooks claim a close occurred when the layout removal was denied (e.g. closing the last base view).

5. MUST apply suggested focus from `remove_view()` deterministically when the closed view was focused or current focus becomes invalid.
   - Enforced in: `LayoutManager::remove_view` (suggestion), `Editor::close_view` (applies suggestion)
   - Tested by: TODO (add regression: test_close_view_focus_uses_overlap_suggestion)
   - Failure symptom: focus jumps to an unintuitive view (first leaf) or becomes invalid and relies on later repairs.

6. MUST implement soft-min sizing for split geometry; MUST not produce zero-sized panes when space allows.
   - Enforced in: `Layout::compute_split_areas` (soft-min policy), `Layout::do_resize_at_path` (same policy during drag)
   - Tested by:
     - `buffer::layout::tests::compute_split_areas_invariants_horizontal`
     - `buffer::layout::tests::compute_split_areas_invariants_vertical`
     - `buffer::layout::tests::compute_split_areas_no_zero_sized_panes`
     - `buffer::layout::tests::compute_split_areas_extreme_position_clamping`
     - TODO (add regression: test_compute_split_areas_soft_min_respected)
   - Failure symptom: panes collapse to width/height 0 on small terminals; hit-testing and cursor rendering desync.

7. MUST cancel an active separator drag if the layout changes or the referenced layer is stale.
   - Enforced in: `LayoutManager::is_drag_stale`, `LayoutManager::cancel_if_stale`
   - Tested by: TODO (add regression: test_drag_cancels_on_layer_generation_change)
   - Failure symptom: dragging resizes the wrong separator or panics due to invalid path/layer after structural changes.

8. MUST bump overlay layer generation when an overlay layer becomes empty (identity ended).
   - Enforced in: `LayoutManager::remove_view` (overlay clear path), `LayoutManager::set_layer` (replacement)
   - Tested by: TODO (add regression: test_overlay_generation_bumps_on_clear)
   - Failure symptom: stale `LayerId` continues to validate and can target a different overlay session.

## Data flow
1. Split (editor command):
   1) Action emits `AppEffect::Split(...)`.
   2) `Editor::{split_*}` computes current view + doc area.
   3) Preflight: `LayoutManager::can_split_horizontal/vertical` returns `(LayerId, view_area)` or `SplitError`.
   4) On success: editor allocates/inserts new `ViewId` buffer, then calls split apply with the preflight `LayerId`.
   5) Focus: editor focuses the new `ViewId`.
   6) Hooks: emit `HookEventData::SplitCreated`.

2. Close view:
   1) Editor checks view exists in some layer (`LayoutManager::layer_of_view`).
   2) Deny close if base and `base_layout.count() <= 1`.
   3) Remove: `LayoutManager::remove_view` mutates the owning layer, returns suggested focus.
   4) Focus: apply suggested focus deterministically if needed.
   5) Hooks/LSP: emit close hooks only after removal succeeds.
   6) Buffer cleanup: remove from buffer store (`finalize_buffer_removal`).
   7) Repairs/redraw: run repairs (should be no-op for windowing invariants) and mark redraw.

3. Separator drag/resize:
   1) Hit-test: `LayoutManager::separator_hit_at_position` produces `SeparatorHit { id: SeparatorId::Split{layer,path}, rect, direction }`.
   2) Drag start: `LayoutManager::start_drag` stores `DragState { id, revision }`.
   3) During drag: `cancel_if_stale` checks `layout_revision` and layer generation/path validity; cancels if stale.
   4) Resize: `LayoutManager::resize_separator` resolves `(layer,path)` into a `Layout::Split` and updates `position` using soft-min clamping.

## Lifecycle
- Base layout:
  - Created with `WindowManager::new(base_layout, focused_view)`.
  - Mutated by split/close operations via `LayoutManager` calls that special-case `LayerId::BASE`.
- Overlay layout slots:
  - Created/replaced via `LayoutManager::set_layer(index, Some(layout))` (always bumps generation).
  - Cleared when overlay becomes empty via `LayoutManager::remove_view` (bumps generation + sets `layout=None`).
  - Accessed via `LayerId` and `validate_layer`/`overlay_layout`.
- Drag state:
  - Starts on separator hit.
  - Cancels if stale (revision changed or layer id invalid).
  - Ends on mouse release.

## Concurrency & ordering
- No internal multithreading is assumed in this subsystem; ordering constraints are about event sequencing and state mutation.
- Ordering requirements:
  - Split: preflight MUST happen before buffer allocation and before layout mutation.
  - Close: layout removal MUST happen before hooks/LSP close.
  - Drag: stale detection MUST happen before applying any resize update.
- `layout_revision`:
  - MUST increment on structural changes (split creation, view removal, layer clear).
  - Used to invalidate drag state across mid-drag structural edits.

## Failure modes & recovery
- Split preflight failure (`SplitError::ViewNotFound`, `SplitError::AreaTooSmall`):
  - Recovery: do not mutate layout; do not allocate buffers; return no-op to caller.
  - Symptom: user command does nothing (optionally message).
- Close denied (attempt to close last base view):
  - Recovery: return false; no hooks; no buffer removal.
  - Symptom: close command is ignored.
- Stale layer reference (`LayerError::*`):
  - Recovery: treat as stale and no-op; cancel drag; ignore resize.
  - Symptom: hover/drag cancels immediately; separator does not move.
- Stale separator path:
  - Recovery: rect lookup returns None; cancel drag; ignore resize.
  - Symptom: drag cancels after a structural change (expected).
- Geometry under tiny terminal sizes:
  - Recovery: soft-min policy degrades to hard mins; split panes remain representable.
  - Symptom: panes become very small but not negative/overflowing; hit-testing remains consistent.

## Recipes
### Add a new overlay layer
Steps:
- Decide a stable overlay slot index for the feature (session-driven overlays typically use a fixed index).
- Build an overlay `buffer::Layout` for that layer.
- Install it:
  - `LayoutManager::set_layer(index, Some(layout))` (returns `LayerId` if the caller needs to store it).
- Use `LayoutManager::top_layer()` or `layer_of_view()` for focus resolution.

### Implement a new split-like operation
Goal: mutate the tree at a specific `ViewId` and focus something deterministic.
Steps:
- Compute `doc_area` and `current_view`.
- Preflight using `LayoutManager::can_split_horizontal/vertical` or an equivalent feasibility check.
- Allocate/insert any new `ViewId` only after preflight success.
- Apply mutation using the preflight `LayerId` (do not recompute layer identity).
- Increment revision (done in layout ops).
- Decide focus target (use `remove_view` suggestion logic or explicit target).
- Emit hooks after mutation.

### Add a new separator interaction
Steps:
- Hit-test: add a new kind of `SeparatorId` variant if needed (keep layer+path validation rules).
- Store in `DragState` and validate via `separator_rect()` or `validate_layer()`.
- Apply resize through `Layout::resize_at_path` (must clamp using soft-min policy).

## Tests
Known invariants/regressions:
- `layout::tests::layer_area_base_only`
- `layout::tests::view_at_position_finds_buffer`
- `buffer::layout::tests::compute_split_areas_invariants_horizontal`
- `buffer::layout::tests::compute_split_areas_invariants_vertical`
- `buffer::layout::tests::compute_split_areas_no_zero_sized_panes`
- `buffer::layout::tests::compute_split_areas_extreme_position_clamping`
- TODO (enumerate remaining): list remaining `test_*` functions from `buffer::layout::tests`.

## Glossary
- Base layer: The split tree owned by `BaseWindow.layout` and addressed by `LayerId::BASE`.
- Overlay layer: A split tree stored in `LayoutManager.layers[idx].layout` for `idx >= 1`.
- Layer slot: A stable index in `LayoutManager.layers` with a generation counter.
- LayerId: `(idx, generation)` handle to a layer slot; prevents stale references after reuse.
- Generation: Monotonic (wrapping) counter incremented when a layer slot is replaced/cleared.
- ViewId: Leaf identity stored in `Layout::Single`; represents an editor view over a document.
- Split: Internal node `Layout::Split { direction, position, first, second }`.
- SplitDirection:
  - `Horizontal`: side-by-side children (vertical divider).
  - `Vertical`: stacked children (horizontal divider).
- SplitPath: Stable path to a split node in a tree (false=first, true=second).
- Separator: The divider between two split children; represented as a rect for hit-testing and resizing.
- Soft-min sizing: Prefer `MIN_WIDTH/MIN_HEIGHT` when space allows; degrade to hard mins when constrained.
