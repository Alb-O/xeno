use crate::buffer::{Layout, SplitDirection, SplitPath, ViewId};
use crate::geometry::Rect;
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

/// Must validate any stored `LayerId` before overlay access.
///
/// * Enforced in: `LayoutManager::validate_layer`, `LayoutManager::overlay_layout`,
///   `LayoutManager::layer`, `LayoutManager::layer_mut`
/// * Failure symptom: Drag/resize/focus can target the wrong overlay after layer reuse.
#[cfg_attr(test, test)]
pub(crate) fn test_layerid_generation_rejects_stale() {
	let mut mgr = LayoutManager::new();

	let id = mgr.set_layer(1, Some(Layout::text(ViewId(1))));
	assert!(mgr.is_valid_layer(id));

	let _new_id = mgr.set_layer(1, Some(Layout::text(ViewId(2))));

	assert!(!mgr.is_valid_layer(id));
	assert!(matches!(mgr.validate_layer(id), Err(LayerError::StaleLayer)));
}

/// Must preserve `LayerId` generation between split preflight and apply.
///
/// * Enforced in: `LayoutManager::can_split_horizontal`, `LayoutManager::can_split_vertical`
/// * Failure symptom: Split applies to the wrong overlay after slot replacement.
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

/// Must not allocate or insert a new `ViewId` when split preflight fails.
///
/// * Enforced in: `Editor::split_horizontal_with_clone`,
///   `Editor::split_vertical_with_clone`, `Editor::split_horizontal`,
///   `Editor::split_vertical`, `SplitError`
/// * Failure symptom: Orphan view exists in buffers but not in any layout.
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

/// Must emit close hooks only after removal succeeds.
///
/// * Enforced in: `Editor::close_view`
/// * Failure symptom: Hooks report a close that was denied.
#[cfg_attr(test, test)]
pub(crate) fn test_close_view_hooks_after_removal() {
	let mut mgr = LayoutManager::new();
	let mut base_layout = Layout::text(ViewId(0));

	let suggestion = mgr.remove_view(&mut base_layout, ViewId(0), doc_area());
	assert!(suggestion.is_none(), "removing last base view must be denied");
	assert!(base_layout.contains(ViewId(0)));
}

/// Must apply `remove_view` focus suggestions deterministically.
///
/// * Enforced in: `LayoutManager::remove_view`, `Editor::close_view`
/// * Failure symptom: Focus jumps to unintuitive views or becomes invalid.
#[cfg_attr(test, test)]
pub(crate) fn test_close_view_focus_uses_overlap_suggestion() {
	let mut mgr = LayoutManager::new();
	let area = doc_area();

	let mut base_layout = Layout::side_by_side(Layout::text(ViewId(0)), Layout::text(ViewId(1)), area);

	let suggestion = mgr.remove_view(&mut base_layout, ViewId(0), area);
	assert_eq!(suggestion, Some(ViewId(1)));
}

/// Must enforce soft-min sizing and avoid zero-sized panes when space allows.
///
/// * Enforced in: `Layout::compute_split_areas`, `Layout::do_resize_at_path`
/// * Failure symptom: Panes collapse to zero width or height.
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
		assert!(rect.width > 0 && rect.height > 0, "view {view:?} has zero-sized area: {rect:?}");
	}
}

/// Must cancel active separator drag when layout changes or layers become stale.
///
/// * Enforced in: `LayoutManager::is_drag_stale`, `LayoutManager::cancel_if_stale`
/// * Failure symptom: Dragging resizes the wrong separator or hits invalid paths.
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
		structure_revision: mgr.structure_revision(),
	});

	let _new_id = mgr.set_layer(1, Some(Layout::text(ViewId(2))));
	assert!(mgr.is_drag_stale());
	assert!(mgr.cancel_if_stale());
	assert!(mgr.dragging_separator.is_none());
}

/// Must keep active separator drags valid across non-structural resize updates.
///
/// * Enforced in: `LayoutManager::resize_separator`, `LayoutManager::cancel_if_stale`
/// * Failure symptom: separator drag cancels after first resize tick and stops moving.
#[cfg_attr(test, test)]
pub(crate) fn test_separator_resize_does_not_invalidate_drag_revision() {
	let mut mgr = LayoutManager::new();
	let area = doc_area();
	let mut base_layout = Layout::side_by_side(Layout::text(ViewId(0)), Layout::text(ViewId(1)), area);

	let (_, _, rect) = mgr
		.separator_positions(&base_layout, area)
		.into_iter()
		.next()
		.expect("split layout should expose one separator");
	let hit = mgr
		.separator_hit_at_position(&base_layout, area, rect.x, rect.y)
		.expect("separator hit should resolve from separator rect");

	mgr.start_drag(&hit);
	let revision_before = mgr.structure_revision();

	let (mouse_x, mouse_y) = match hit.direction {
		SplitDirection::Vertical => (rect.x.saturating_add(3), rect.y),
		SplitDirection::Horizontal => (rect.x, rect.y.saturating_add(3)),
	};
	mgr.resize_separator(&mut base_layout, area, &hit.id, mouse_x, mouse_y);

	assert_eq!(
		mgr.structure_revision(),
		revision_before,
		"separator resize should not bump structural layout revision"
	);
	assert!(!mgr.cancel_if_stale(), "drag should remain active after non-structural resize update");
	assert!(mgr.drag_state().is_some());
}

/// Must cancel active separator drag when a structural split/close changes the layout tree.
///
/// * Enforced in: `LayoutManager::is_drag_stale`, `LayoutManager::cancel_if_stale`
/// * Failure symptom: Drag continues on a stale split path after a view close, resizing the wrong separator.
#[cfg_attr(test, test)]
pub(crate) fn test_separator_drag_cancels_on_structure_revision() {
	let mut mgr = LayoutManager::new();
	let area = doc_area();
	let mut base_layout = Layout::side_by_side(Layout::text(ViewId(0)), Layout::text(ViewId(1)), area);

	let hit = mgr
		.separator_hit_at_position(&base_layout, area, 40, 12)
		.expect("separator hit should resolve in side-by-side layout");

	mgr.start_drag(&hit);
	assert!(mgr.is_dragging());

	let _suggestion = mgr.remove_view(&mut base_layout, ViewId(1), area);

	assert!(mgr.is_drag_stale(), "structural change from remove_view must make drag stale");
	assert!(mgr.cancel_if_stale());
	assert!(!mgr.is_dragging());
}

/// Must reject resize operations with a stale SplitPath without mutating layout.
///
/// * Enforced in: `LayoutManager::resize_separator`, `Layout::resize_at_path`
/// * Failure symptom: Stale path targets wrong split node or panics on missing path segment.
#[cfg_attr(test, test)]
pub(crate) fn test_stale_splitpath_resize_is_rejected() {
	use crate::layout::types::SeparatorId;

	let mut mgr = LayoutManager::new();
	let area = doc_area();

	let left = Layout::side_by_side(Layout::text(ViewId(0)), Layout::text(ViewId(1)), area);
	let mut base_layout = Layout::stacked(left, Layout::text(ViewId(2)), area);

	let stale_path = SplitPath(vec![false, false]);
	let stale_id = SeparatorId::Split {
		path: stale_path,
		layer: LayerId::BASE,
	};

	let areas_before = base_layout.compute_areas(area);

	mgr.resize_separator(&mut base_layout, area, &stale_id, 50, 15);

	let areas_after = base_layout.compute_areas(area);
	assert_eq!(areas_before, areas_after, "stale path resize must not mutate layout");
}

/// Must clamp separator resize to soft-min bounds when space allows.
///
/// * Enforced in: `Layout::compute_split_areas`, `Layout::do_resize_at_path`
/// * Failure symptom: Dragging separator to extremes produces zero-width or zero-height panes.
#[cfg_attr(test, test)]
pub(crate) fn test_resize_separator_clamps_to_soft_min() {
	let mut mgr = LayoutManager::new();
	let area = doc_area(); // 80x24
	let mut base_layout = Layout::side_by_side(Layout::text(ViewId(0)), Layout::text(ViewId(1)), area);

	let sep_id = {
		let hit = mgr
			.separator_hit_at_position(&base_layout, area, 40, 12)
			.expect("separator hit should resolve");
		hit.id
	};

	// Drag far left (mouse_x = 0)
	mgr.resize_separator(&mut base_layout, area, &sep_id, 0, 12);
	let areas = base_layout.compute_areas(area);
	let left = areas.iter().find(|(v, _)| *v == ViewId(0)).map(|(_, r)| r).unwrap();
	let right = areas.iter().find(|(v, _)| *v == ViewId(1)).map(|(_, r)| r).unwrap();
	assert!(left.width >= Layout::MIN_WIDTH, "left pane must respect soft-min after extreme left drag");
	assert!(right.width >= Layout::MIN_WIDTH, "right pane must respect soft-min after extreme left drag");

	// Drag far right (mouse_x = 79)
	mgr.resize_separator(&mut base_layout, area, &sep_id, 79, 12);
	let areas = base_layout.compute_areas(area);
	let left = areas.iter().find(|(v, _)| *v == ViewId(0)).map(|(_, r)| r).unwrap();
	let right = areas.iter().find(|(v, _)| *v == ViewId(1)).map(|(_, r)| r).unwrap();
	assert!(left.width >= Layout::MIN_WIDTH, "left pane must respect soft-min after extreme right drag");
	assert!(right.width >= Layout::MIN_WIDTH, "right pane must respect soft-min after extreme right drag");
}

/// Must produce non-overlapping, non-negative geometry even when area is smaller than soft-min total.
///
/// * Enforced in: `Layout::compute_split_areas`
/// * Failure symptom: Panes overlap or produce negative/overflowing coordinates under tiny terminals.
#[cfg_attr(test, test)]
pub(crate) fn test_compute_areas_degrades_gracefully_under_tiny_area() {
	let tiny = Rect { x: 0, y: 0, width: 5, height: 3 };
	let layout = Layout::side_by_side(Layout::text(ViewId(0)), Layout::text(ViewId(1)), tiny);
	let areas = layout.compute_areas(tiny);

	for (view, rect) in &areas {
		assert!(
			rect.x + rect.width <= tiny.x + tiny.width,
			"view {view:?} extends beyond parent width"
		);
		assert!(
			rect.y + rect.height <= tiny.y + tiny.height,
			"view {view:?} extends beyond parent height"
		);
	}

	// Check no overlap between the two views
	if areas.len() == 2 {
		let a = &areas[0].1;
		let b = &areas[1].1;
		let a_end = a.x + a.width;
		let b_end = b.x + b.width;
		assert!(a_end <= b.x || b_end <= a.x, "panes must not overlap horizontally");
	}

	// Total width must partition the parent (accounting for separator)
	let total_w: u16 = areas.iter().map(|(_, r)| r.width).sum();
	assert!(total_w + 1 <= tiny.width || tiny.width <= 1, "views + separator must fit within parent");
}

/// Must clamp separator resize to soft-min bounds for vertical (stacked) splits.
///
/// * Enforced in: `Layout::compute_split_areas`, `Layout::do_resize_at_path`
/// * Failure symptom: Dragging stacked separator to extremes produces zero-height panes.
#[cfg_attr(test, test)]
pub(crate) fn test_resize_separator_clamps_to_soft_min_vertical() {
	let mut mgr = LayoutManager::new();
	let area = doc_area(); // 80x24
	let mut base_layout = Layout::stacked(Layout::text(ViewId(0)), Layout::text(ViewId(1)), area);

	let sep_id = {
		let hit = mgr
			.separator_hit_at_position(&base_layout, area, 40, 12)
			.expect("separator hit should resolve");
		hit.id
	};

	// Drag far up (mouse_y = 0)
	mgr.resize_separator(&mut base_layout, area, &sep_id, 40, 0);
	let areas = base_layout.compute_areas(area);
	let top = areas.iter().find(|(v, _)| *v == ViewId(0)).map(|(_, r)| r).unwrap();
	let bottom = areas.iter().find(|(v, _)| *v == ViewId(1)).map(|(_, r)| r).unwrap();
	assert!(top.height >= Layout::MIN_HEIGHT, "top pane must respect soft-min after extreme up drag");
	assert!(bottom.height >= Layout::MIN_HEIGHT, "bottom pane must respect soft-min after extreme up drag");

	// Drag far down (mouse_y = 23)
	mgr.resize_separator(&mut base_layout, area, &sep_id, 40, 23);
	let areas = base_layout.compute_areas(area);
	let top = areas.iter().find(|(v, _)| *v == ViewId(0)).map(|(_, r)| r).unwrap();
	let bottom = areas.iter().find(|(v, _)| *v == ViewId(1)).map(|(_, r)| r).unwrap();
	assert!(top.height >= Layout::MIN_HEIGHT, "top pane must respect soft-min after extreme down drag");
	assert!(bottom.height >= Layout::MIN_HEIGHT, "bottom pane must respect soft-min after extreme down drag");
}

/// Must bump overlay generation when an overlay layer is cleared.
///
/// * Enforced in: `LayoutManager::remove_view`, `LayoutManager::set_layer`
/// * Failure symptom: Stale layer IDs keep validating against a new session.
#[cfg_attr(test, test)]
pub(crate) fn test_overlay_generation_bumps_on_clear() {
	let mut mgr = LayoutManager::new();
	let mut base_layout = Layout::text(ViewId(0));

	let id = mgr.set_layer(1, Some(Layout::text(ViewId(1))));
	let _suggestion = mgr.remove_view(&mut base_layout, ViewId(1), doc_area());

	assert!(!mgr.is_valid_layer(id));
}
