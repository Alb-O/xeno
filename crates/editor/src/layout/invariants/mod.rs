//! Machine-checkable invariant catalog and proof entrypoints for layout/windowing.
#![allow(dead_code)]

pub(crate) mod catalog;

#[allow(unused_imports)]
pub(crate) use catalog::{
	APPLY_REMOVE_VIEW_FOCUS_SUGGESTION_DETERMINISTICALLY, BUMP_OVERLAY_GENERATION_ON_LAYER_CLEAR,
	CANCEL_STALE_SEPARATOR_DRAG, EMIT_CLOSE_HOOKS_AFTER_SUCCESSFUL_REMOVAL,
	NO_ORPHAN_VIEW_ON_FAILED_SPLIT_PREFLIGHT, PRESERVE_LAYER_GENERATION_FROM_PREFLIGHT_TO_APPLY,
	SOFT_MIN_SPLIT_GEOMETRY_PREVENTS_ZERO_PANES, VALIDATE_LAYER_ID_BEFORE_OVERLAY_ACCESS,
};

#[cfg(doc)]
pub(crate) fn test_layerid_generation_rejects_stale() {}

#[cfg(doc)]
pub(crate) fn test_split_preflight_apply_generation_preserved() {}

#[cfg(doc)]
pub(crate) fn test_split_preflight_no_orphan_buffer() {}

#[cfg(doc)]
pub(crate) fn test_close_view_hooks_after_removal() {}

#[cfg(doc)]
pub(crate) fn test_close_view_focus_uses_overlap_suggestion() {}

#[cfg(doc)]
pub(crate) fn test_compute_split_areas_soft_min_respected() {}

#[cfg(doc)]
pub(crate) fn test_drag_cancels_on_layer_generation_change() {}

#[cfg(doc)]
pub(crate) fn test_overlay_generation_bumps_on_clear() {}

#[cfg(test)]
mod proofs;

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use proofs::{
	test_close_view_focus_uses_overlap_suggestion, test_close_view_hooks_after_removal,
	test_compute_split_areas_soft_min_respected, test_drag_cancels_on_layer_generation_change,
	test_layerid_generation_rejects_stale, test_overlay_generation_bumps_on_clear,
	test_split_preflight_apply_generation_preserved, test_split_preflight_no_orphan_buffer,
};
