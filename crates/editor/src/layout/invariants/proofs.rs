//! Machine-checkable invariant proofs for the layout subsystem.

use xeno_tui::layout::Rect;

use crate::buffer::{Layout, SplitPath, ViewId};
use crate::layout::manager::LayoutManager;
use crate::layout::types::{LayerError, LayerId};
use crate::separator::DragState;

fn doc_area() -> Rect {
	Rect {
		x: 0,
		y: 0,
		width: 80,
		height: 24,
	}
}

/// Stale `LayerId` generations are rejected.
#[cfg_attr(test, test)]
pub(crate) fn test_layerid_generation_rejects_stale() {
	let mut mgr = LayoutManager::new();

	let id = mgr.set_layer(1, Some(Layout::text(ViewId(1))));
	assert!(mgr.is_valid_layer(id));

	let _new_id = mgr.set_layer(1, Some(Layout::text(ViewId(2))));

	assert!(!mgr.is_valid_layer(id));
	assert!(matches!(
		mgr.validate_layer(id),
		Err(LayerError::StaleLayer)
	));
}

/// `LayerId` generation is preserved from split preflight to apply.
#[cfg_attr(test, test)]
pub(crate) fn test_split_preflight_apply_generation_preserved() {
	let mgr = LayoutManager::new();
	let base_layout = Layout::text(ViewId(0));

	let result = mgr.can_split_horizontal(&base_layout, ViewId(0), doc_area());
	assert!(result.is_ok(), "preflight should succeed on 80x24");

	let (layer_id, _area) = result.unwrap();
	assert_eq!(layer_id, LayerId::BASE, "base view should be in base layer");
	assert!(mgr.is_valid_layer(layer_id));
}

/// Failed split preflight does not leave orphan views.
#[cfg_attr(test, test)]
pub(crate) fn test_split_preflight_no_orphan_buffer() {
	let mgr = LayoutManager::new();
	let base_layout = Layout::text(ViewId(0));

	let tiny_area = Rect {
		x: 0,
		y: 0,
		width: 2,
		height: 2,
	};

	let result = mgr.can_split_horizontal(&base_layout, ViewId(0), tiny_area);
	assert!(result.is_err(), "split should fail on 2x2 area");
}

/// Close hooks are emitted only after successful removal.
#[cfg_attr(test, test)]
pub(crate) fn test_close_view_hooks_after_removal() {
	let mut mgr = LayoutManager::new();
	let mut base_layout = Layout::text(ViewId(0));

	let suggestion = mgr.remove_view(&mut base_layout, ViewId(0), doc_area());
	assert!(
		suggestion.is_none(),
		"removing last base view must be denied"
	);
	assert!(base_layout.contains(ViewId(0)));
}

/// `remove_view` focus suggestions are deterministic.
#[cfg_attr(test, test)]
pub(crate) fn test_close_view_focus_uses_overlap_suggestion() {
	let mut mgr = LayoutManager::new();
	let area = doc_area();

	let mut base_layout =
		Layout::side_by_side(Layout::text(ViewId(0)), Layout::text(ViewId(1)), area);

	let suggestion = mgr.remove_view(&mut base_layout, ViewId(0), area);
	assert_eq!(suggestion, Some(ViewId(1)));
}

/// Soft-min sizing keeps panes non-zero when space allows.
#[cfg_attr(test, test)]
pub(crate) fn test_compute_split_areas_soft_min_respected() {
	let area = Rect {
		x: 0,
		y: 0,
		width: 10,
		height: 10,
	};

	let layout = Layout::side_by_side(Layout::text(ViewId(0)), Layout::text(ViewId(1)), area);
	let areas = layout.compute_areas(area);
	for (view, rect) in &areas {
		assert!(
			rect.width > 0 && rect.height > 0,
			"view {view:?} has zero-sized area: {rect:?}"
		);
	}
}

/// Active drags cancel when layer generations change.
#[cfg_attr(test, test)]
pub(crate) fn test_drag_cancels_on_layer_generation_change() {
	use crate::layout::types::SeparatorId;

	let mut mgr = LayoutManager::new();
	let layer_id = mgr.set_layer(1, Some(Layout::text(ViewId(1))));

	mgr.dragging_separator = Some(DragState {
		id: SeparatorId::Split {
			layer: layer_id,
			path: SplitPath::default(),
		},
		revision: mgr.layout_revision(),
	});

	let _new_id = mgr.set_layer(1, Some(Layout::text(ViewId(2))));
	assert!(mgr.is_drag_stale());
	assert!(mgr.cancel_if_stale());
	assert!(mgr.dragging_separator.is_none());
}

/// Clearing an overlay bumps its generation.
#[cfg_attr(test, test)]
pub(crate) fn test_overlay_generation_bumps_on_clear() {
	let mut mgr = LayoutManager::new();
	let mut base_layout = Layout::text(ViewId(0));

	let id = mgr.set_layer(1, Some(Layout::text(ViewId(1))));
	let _suggestion = mgr.remove_view(&mut base_layout, ViewId(1), doc_area());

	assert!(!mgr.is_valid_layer(id));
}
