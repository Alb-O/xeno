//! Invariant catalog for [`crate::layout::manager::LayoutManager`].
#![allow(dead_code)]

/// Must validate any stored [`crate::layout::types::LayerId`] before overlay access.
///
/// - Enforced in: [`crate::layout::manager::LayoutManager::validate_layer`], [`crate::layout::manager::LayoutManager::overlay_layout`], [`crate::layout::manager::LayoutManager::layer`], [`crate::layout::manager::LayoutManager::layer_mut`]
/// - Tested by: [`crate::layout::invariants::test_layerid_generation_rejects_stale`]
/// - Failure symptom: Drag/resize/focus can target the wrong overlay after layer reuse.
pub(crate) const VALIDATE_LAYER_ID_BEFORE_OVERLAY_ACCESS: () = ();

/// Must preserve [`crate::layout::types::LayerId`] generation between split preflight and apply.
///
/// - Enforced in: [`crate::layout::manager::LayoutManager::can_split_horizontal`], [`crate::layout::manager::LayoutManager::can_split_vertical`]
/// - Tested by: [`crate::layout::invariants::test_split_preflight_apply_generation_preserved`]
/// - Failure symptom: Split applies to the wrong overlay after slot replacement.
pub(crate) const PRESERVE_LAYER_GENERATION_FROM_PREFLIGHT_TO_APPLY: () = ();

/// Must not allocate or insert a new [`crate::buffer::ViewId`] when split preflight fails.
///
/// - Enforced in: [`crate::impls::Editor::split_horizontal_with_clone`], [`crate::impls::Editor::split_vertical_with_clone`], [`crate::impls::Editor::split_horizontal`], [`crate::impls::Editor::split_vertical`], [`crate::layout::splits::SplitError`]
/// - Tested by: [`crate::layout::invariants::test_split_preflight_no_orphan_buffer`]
/// - Failure symptom: Orphan view exists in buffers but not in any layout.
pub(crate) const NO_ORPHAN_VIEW_ON_FAILED_SPLIT_PREFLIGHT: () = ();

/// Must emit close hooks only after removal succeeds.
///
/// - Enforced in: [`crate::impls::Editor::close_view`]
/// - Tested by: [`crate::layout::invariants::test_close_view_hooks_after_removal`]
/// - Failure symptom: Hooks report a close that was denied.
pub(crate) const EMIT_CLOSE_HOOKS_AFTER_SUCCESSFUL_REMOVAL: () = ();

/// Must apply [`crate::layout::manager::LayoutManager::remove_view`] focus suggestions deterministically.
///
/// - Enforced in: [`crate::layout::manager::LayoutManager::remove_view`], [`crate::impls::Editor::close_view`]
/// - Tested by: [`crate::layout::invariants::test_close_view_focus_uses_overlap_suggestion`]
/// - Failure symptom: Focus jumps to unintuitive views or becomes invalid.
pub(crate) const APPLY_REMOVE_VIEW_FOCUS_SUGGESTION_DETERMINISTICALLY: () = ();

/// Must enforce soft-min sizing and avoid zero-sized panes when space allows.
///
/// - Enforced in: [`crate::buffer::Layout::compute_split_areas`], [`crate::buffer::Layout::do_resize_at_path`]
/// - Tested by: [`crate::layout::invariants::test_compute_split_areas_soft_min_respected`]
/// - Failure symptom: Panes collapse to zero width or height.
pub(crate) const SOFT_MIN_SPLIT_GEOMETRY_PREVENTS_ZERO_PANES: () = ();

/// Must cancel active separator drag when layout changes or referenced layers become stale.
///
/// - Enforced in: [`crate::layout::manager::LayoutManager::is_drag_stale`], [`crate::layout::manager::LayoutManager::cancel_if_stale`]
/// - Tested by: [`crate::layout::invariants::test_drag_cancels_on_layer_generation_change`]
/// - Failure symptom: Dragging resizes the wrong separator or hits invalid paths.
pub(crate) const CANCEL_STALE_SEPARATOR_DRAG: () = ();

/// Must bump overlay generation when an overlay layer is cleared.
///
/// - Enforced in: [`crate::layout::manager::LayoutManager::remove_view`], [`crate::layout::manager::LayoutManager::set_layer`]
/// - Tested by: [`crate::layout::invariants::test_overlay_generation_bumps_on_clear`]
/// - Failure symptom: Stale layer IDs keep validating against a new session.
pub(crate) const BUMP_OVERLAY_GENERATION_ON_LAYER_CLEAR: () = ();
