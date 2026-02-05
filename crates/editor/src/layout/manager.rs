//! Windowing and split layout system.
//!
//! # Purpose
//!
//! Owns window containers (base + floating), stacked layout layers, split-tree geometry ([`Layout`](crate::buffer::Layout)), view navigation, separator hit/resize/drag state, and editor-level split/close integration.
//!
//! Does not own: buffer/document content (owned by buffer/document subsystems), UI widget styling (owned by renderer + widget layer), overlay session policy (owned by overlay system; windowing only hosts floating windows and overlay layouts).
//!
//! Source of truth:
//! - Layout/layers/splits/navigation: [`LayoutManager`] + [`Layout`](crate::buffer::Layout)
//! - Base/floating window containers: [`WindowManager`](crate::window::manager::WindowManager),
//!   [`BaseWindow`](crate::window::types::BaseWindow),
//!   [`FloatingWindow`](crate::window::types::FloatingWindow)
//! - Editor integration: `Editor` split/close methods
//!
//! # Mental model
//!
//! - There is exactly one base split tree (owned by `BaseWindow.layout`).
//! - There are zero or more overlay split trees (owned by `LayoutManager.layers: Vec<LayerSlot>`).
//! - Every leaf in any split tree is a [`ViewId`](crate::buffer::ViewId).
//! - Any reference to an overlay layer that can outlive the immediate call stack MUST use `LayerId { idx, generation }` and MUST be validated before use.
//! - A separator is identified by `(LayerId, SplitPath)`; resizing moves
//!   `Layout::Split.position` at that path.
//! - Split operations are atomic at editor level: feasibility is checked before buffer
//!   allocation and before mutating layout.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`WindowManager`] | Owns all windows (base + floating) | MUST always contain exactly one base window | `WindowManager::new`, `WindowManager::create_floating`, `WindowManager::close_floating` |
//! | [`BaseWindow`] | Base split tree container | `layout` is the base layer tree; `focused_buffer` MUST be a view in the base tree after repairs | editor state |
//! | [`FloatingWindow`] | Absolute-positioned overlay window | Pure data; policy fields are enforced elsewhere | `FloatingWindow::new` |
//! | [`LayoutManager`] | Owns overlay layers + separator interaction | `layers[0]` is a dummy slot; base layout lives outside slots | `LayoutManager` + `layout::*` methods |
//! | [`LayerId`] | Generational layer handle | MUST validate before deref unless `is_base()` | `layout::types` + `layout::layers` |
//! | [`LayerSlot`] | Storage slot for overlay layer | `generation` MUST bump when layer identity ends (clear/replace) | `LayoutManager::set_layer`, `LayoutManager::remove_view` |
//! | [`LayerError`] | Layer validation failure | MUST treat as stale/invalid and no-op/cancel | `LayoutManager::validate_layer` |
//! | [`Layout`] | Split tree for arranging [`ViewId`] leaves | `position` is parent-local; geometry MUST obey soft-min policy | `buffer::layout` + `buffer::layout::areas` |
//! | [`SplitPath`] | Stable path to a split node | Path is relative to the current tree shape; stale paths MUST be rejected | `buffer::layout::areas` path APIs |
//! | [`SeparatorId`] | Persistent separator identity | MUST validate layer generation + path before resize | `layout::separators` + `layout::drag` |
//! | [`SplitError`] | Split preflight failure | MUST not allocate/insert buffers when preflight fails | `layout::splits` + `impls::splits` |
//! | [`DragState`] | Active separator drag | MUST cancel when revision or layer generation invalidates id | `layout::drag` |
//! | [`ViewId`] | Leaf identity in layouts | A [`ViewId`] MUST not exist in multiple layers simultaneously | enforced by editor invariants/repair |
//!
//! # Invariants
//!
//! 1. MUST validate any stored `LayerId` before accessing an overlay layout.
//!    - Enforced in: `LayoutManager::validate_layer`, `LayoutManager::overlay_layout`, `LayoutManager::overlay_layout_mut`, `LayoutManager::layer`, `LayoutManager::layer_mut`
//!    - Tested by: TODO (add regression: test_layerid_generation_rejects_stale)
//!    - Failure symptom: separator drag/resize or focus targets operate on the wrong overlay after a layer is cleared/reused.
//!
//! 2. MUST preserve `LayerId` generation across split preflight to apply.
//!    - Enforced in: `LayoutManager::can_split_horizontal`, `LayoutManager::can_split_vertical` and split apply APIs taking `LayerId`
//!    - Tested by: TODO (add regression: test_split_preflight_apply_generation_preserved)
//!    - Failure symptom: split applies to the wrong layer if the overlay slot is replaced between check and apply.
//!
//! 3. MUST NOT allocate/insert a new `ViewId` for a split if the split cannot be created.
//!    - Enforced in: `Editor::split_horizontal_with_clone`, `Editor::split_vertical_with_clone`, `Editor::split_horizontal`, `Editor::split_vertical` (preflight before buffer creation) and `SplitError`
//!    - Tested by: TODO (add regression: test_split_preflight_no_orphan_buffer)
//!    - Failure symptom: orphan `ViewId` exists in buffer store but not in any layout; focus may jump to a non-rendered view.
//!
//! 4. MUST emit close hooks only after the view has been removed from layout successfully.
//!    - Enforced in: `Editor::close_view`
//!    - Tested by: TODO (add regression: test_close_view_hooks_after_removal)
//!    - Failure symptom: hooks claim a close occurred when the layout removal was denied
//!      (e.g. closing the last base view).
//!
//! 5. MUST apply suggested focus from `remove_view()` deterministically when the closed view was focused or current focus becomes invalid.
//!    - Enforced in: `LayoutManager::remove_view` (suggestion), `Editor::close_view` (applies suggestion)
//!    - Tested by: TODO (add regression: test_close_view_focus_uses_overlap_suggestion)
//!    - Failure symptom: focus jumps to an unintuitive view (first leaf) or becomes invalid and relies on later repairs.
//!
//! 6. MUST implement soft-min sizing for split geometry; MUST not produce zero-sized panes when space allows.
//!    - Enforced in: `Layout::compute_split_areas` (soft-min policy), `Layout::do_resize_at_path` (same policy during drag)
//!    - Tested by:
//!      - `buffer::layout::tests::compute_split_areas_invariants_horizontal`
//!      - `buffer::layout::tests::compute_split_areas_invariants_vertical`
//!      - `buffer::layout::tests::compute_split_areas_no_zero_sized_panes`
//!      - `buffer::layout::tests::compute_split_areas_extreme_position_clamping`
//!      - TODO (add regression: test_compute_split_areas_soft_min_respected)
//!    - Failure symptom: panes collapse to width/height 0 on small terminals; hit-testing and cursor rendering desync.
//!
//! 7. MUST cancel an active separator drag if the layout changes or the referenced layer is stale.
//!    - Enforced in: `LayoutManager::is_drag_stale`, `LayoutManager::cancel_if_stale`
//!    - Tested by: TODO (add regression: test_drag_cancels_on_layer_generation_change)
//!    - Failure symptom: dragging resizes the wrong separator or panics due to invalid path/layer after structural changes.
//!
//! 8. MUST bump overlay layer generation when an overlay layer becomes empty (identity ended).
//!    - Enforced in: `LayoutManager::remove_view` (overlay clear path), `LayoutManager::set_layer` (replacement)
//!    - Tested by: TODO (add regression: test_overlay_generation_bumps_on_clear)
//!    - Failure symptom: stale `LayerId` continues to validate and can target a different overlay session.
//!
//! # Data flow
//!
//! ## Split (editor command)
//!
//! 1. Action emits `AppEffect::Split(...)`.
//! 2. `Editor::{split_*}` computes current view + doc area.
//! 3. Preflight: `LayoutManager::can_split_horizontal`/`can_split_vertical` returns
//!    `(LayerId, view_area)` or `SplitError`.
//! 4. On success: editor allocates/inserts new `ViewId` buffer, then calls split apply with
//!    the preflight `LayerId`.
//! 5. Focus: editor focuses the new `ViewId`.
//! 6. Hooks: emit `HookEventData::SplitCreated`.
//!
//! ## Close view
//!
//! 1. Editor checks view exists in some layer (`LayoutManager::layer_of_view`).
//! 2. Deny close if base and `base_layout.count() <= 1`.
//! 3. Remove: `LayoutManager::remove_view` mutates the owning layer, returns suggested focus.
//! 4. Focus: apply suggested focus deterministically if needed.
//! 5. Hooks/LSP: emit close hooks only after removal succeeds.
//! 6. Buffer cleanup: remove from buffer store (`finalize_buffer_removal`).
//! 7. Repairs/redraw: run repairs (should be no-op for windowing invariants) and mark redraw.
//!
//! ## Separator drag/resize
//!
//! 1. Hit-test: `LayoutManager::separator_hit_at_position` produces
//!    `SeparatorHit { id: SeparatorId::Split{layer,path}, rect, direction }`.
//! 2. Drag start: `LayoutManager::start_drag` stores `DragState { id, revision }`.
//! 3. During drag: `cancel_if_stale` checks `layout_revision` and layer generation/path validity; cancels if stale.
//! 4. Resize: `LayoutManager::resize_separator` resolves `(layer,path)` into a `Layout::Split` and updates `position` using soft-min clamping.
//!
//! # Lifecycle
//!
//! ## Base layout
//!
//! - Created with `WindowManager::new(base_layout, focused_view)`.
//! - Mutated by split/close operations via `LayoutManager` calls that special-case
//!   `LayerId::BASE`.
//!
//! ## Overlay layout slots
//!
//! - Created/replaced via `LayoutManager::set_layer(index, Some(layout))` (always bumps generation).
//! - Cleared when overlay becomes empty via `LayoutManager::remove_view` (bumps generation + sets `layout=None`).
//! - Accessed via `LayerId` and `validate_layer`/`overlay_layout`.
//!
//! ## Drag state
//!
//! - Starts on separator hit.
//! - Cancels if stale (revision changed or layer id invalid).
//! - Ends on mouse release.
//!
//! # Concurrency and ordering
//!
//! No internal multithreading is assumed in this subsystem; ordering constraints are about event sequencing and state mutation.
//!
//! Ordering requirements:
//! - Split: preflight MUST happen before buffer allocation and before layout mutation.
//! - Close: layout removal MUST happen before hooks/LSP close.
//! - Drag: stale detection MUST happen before applying any resize update.
//!
//! `layout_revision`: MUST increment on structural changes (split creation, view removal, layer clear).
//! - Enforced in: `increment_revision` calls in `splits.rs` (split apply) and `separators.rs` (resize structural changes).
//!
//! # Failure modes and recovery
//!
//! - Split preflight failure (`SplitError::ViewNotFound`, `SplitError::AreaTooSmall`):
//!   Recovery: do not mutate layout; do not allocate buffers; return no-op to caller.
//!   Symptom: user command does nothing (optionally message).
//! - Close denied (attempt to close last base view):
//!   Recovery: return false; no hooks; no buffer removal.
//!   Symptom: close command is ignored.
//! - Stale layer reference (`LayerError::*`):
//!   Recovery: treat as stale and no-op; cancel drag; ignore resize.
//!   Symptom: hover/drag cancels immediately; separator does not move.
//! - Stale separator path: Recovery: rect lookup returns None; cancel drag; ignore resize. Symptom: drag cancels after a structural change (expected).
//! - Geometry under tiny terminal sizes: Recovery: soft-min policy degrades to hard mins; split panes remain representable. Symptom: panes become very small but not negative/overflowing; hit-testing remains consistent.
//!
//! # Recipes
//!
//! ## Add a new overlay layer
//!
//! 1. Decide a stable overlay slot index for the feature (session-driven overlays typically use a fixed index).
//! 2. Build an overlay [`Layout`](crate::buffer::Layout) for that layer.
//! 3. Install it: `LayoutManager::set_layer(index, Some(layout))` (returns `LayerId` if the caller needs to store it).
//! 4. Use `LayoutManager::top_layer()` or `layer_of_view()` for focus resolution.
//!
//! ## Implement a new split-like operation
//!
//! Goal: mutate the tree at a specific `ViewId` and focus something deterministic.
//!
//! 1. Compute `doc_area` and `current_view`.
//! 2. Preflight using `LayoutManager::can_split_horizontal`/`can_split_vertical` or an equivalent feasibility check.
//! 3. Allocate/insert any new `ViewId` only after preflight success.
//! 4. Apply mutation using the preflight `LayerId` (do not recompute layer identity).
//! 5. Increment revision (done in layout ops).
//! 6. Decide focus target (use `remove_view` suggestion logic or explicit target).
//! 7. Emit hooks after mutation.
//!
//! ## Add a new separator interaction
//!
//! 1. Hit-test: add a new kind of `SeparatorId` variant if needed (keep layer+path validation rules).
//! 2. Store in `DragState` and validate via `separator_rect()` or `validate_layer()`.
//! 3. Apply resize through `Layout::resize_at_path` (must clamp using soft-min policy).
//!
use xeno_tui::layout::Rect;

use crate::buffer::{SplitDirection, ViewId};
use crate::layout::types::LayerSlot;
use crate::separator::{DragState, MouseVelocityTracker, SeparatorHoverAnimation};

/// Manages stacked layout layers and separator interactions.
///
/// Layouts are organized in ordered layers. Layer 0 is the base layout
/// owned by the base window; higher layers overlay on top with
/// transparent backgrounds.
///
/// Overlay layers use generational tracking to prevent stale references.
pub struct LayoutManager {
	/// Layout layers above the base layout (index 0 reserved for base).
	/// Uses generational slots to prevent stale references.
	pub(super) layers: Vec<LayerSlot>,

	/// Revision counter incremented on structural layout changes.
	///
	/// Used to detect stale drag state when the layout changes mid-drag
	/// (e.g., a view is closed while dragging a separator).
	layout_revision: u64,

	/// Currently hovered separator (for visual feedback during resize).
	pub hovered_separator: Option<(SplitDirection, Rect)>,

	/// Separator the mouse is currently over (regardless of velocity).
	pub separator_under_mouse: Option<(SplitDirection, Rect)>,

	/// Animation state for separator hover fade effects.
	pub separator_hover_animation: Option<SeparatorHoverAnimation>,

	/// Tracks mouse velocity to suppress hover effects during fast movement.
	pub mouse_velocity: MouseVelocityTracker,

	/// Active separator drag state for resizing splits.
	pub dragging_separator: Option<DragState>,

	/// Tracks the view where a text selection drag started.
	pub text_selection_origin: Option<(ViewId, Rect)>,
}

impl Default for LayoutManager {
	fn default() -> Self {
		Self {
			layers: vec![LayerSlot::empty()],
			layout_revision: 0,
			hovered_separator: None,
			separator_under_mouse: None,
			separator_hover_animation: None,
			mouse_velocity: MouseVelocityTracker::default(),
			dragging_separator: None,
			text_selection_origin: None,
		}
	}
}

impl LayoutManager {
	/// Creates a new `LayoutManager` without owning the base layout.
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns the current layout revision.
	///
	/// Structural changes (splits, removals) increment this value.
	pub fn layout_revision(&self) -> u64 {
		self.layout_revision
	}

	/// Increments the layout revision counter.
	///
	/// Call this after any structural change to the layout.
	pub(super) fn increment_revision(&mut self) {
		self.layout_revision = self.layout_revision.wrapping_add(1);
	}
}
