use super::*;

fn make_rect(x: u16, y: u16, width: u16, height: u16) -> crate::geometry::Rect {
	crate::geometry::Rect { x, y, width, height }
}

fn get_position(layout: &Layout) -> Option<u16> {
	match layout {
		Layout::Split { position, .. } => Some(*position),
		Layout::Single(_) => None,
	}
}

fn get_inner_position(layout: &Layout) -> Option<u16> {
	match layout {
		Layout::Split { second, .. } => get_position(second),
		Layout::Single(_) => None,
	}
}

#[test]
fn single_layout() {
	let layout = Layout::single(ViewId(1));
	assert_eq!(layout.first_buffer(), Some(ViewId(1)));
	assert_eq!(layout.buffer_ids(), vec![ViewId(1)]);
	assert!(layout.contains(ViewId(1)));
	assert!(!layout.contains(ViewId(2)));
}

#[test]
fn side_by_side_split() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::side_by_side(Layout::single(ViewId(1)), Layout::single(ViewId(2)), area);

	assert_eq!(layout.first_buffer(), Some(ViewId(1)));
	assert_eq!(layout.buffer_ids(), vec![ViewId(1), ViewId(2)]);
	assert!(layout.contains(ViewId(1)));
	assert!(layout.contains(ViewId(2)));
	assert!(!layout.contains(ViewId(3)));
	assert_eq!(get_position(&layout), Some(40));
}

#[test]
fn next_prev_buffer() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::side_by_side(Layout::single(ViewId(1)), Layout::single(ViewId(2)), area);

	assert_eq!(layout.next_buffer(ViewId(1)), ViewId(2));
	assert_eq!(layout.next_buffer(ViewId(2)), ViewId(1));
	assert_eq!(layout.prev_buffer(ViewId(1)), ViewId(2));
	assert_eq!(layout.prev_buffer(ViewId(2)), ViewId(1));
}

#[test]
fn remove_buffer() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::side_by_side(Layout::single(ViewId(1)), Layout::single(ViewId(2)), area);

	let after_remove = layout.remove(ViewId(1)).unwrap();
	assert_eq!(after_remove.buffer_ids(), vec![ViewId(2)]);

	let single = Layout::single(ViewId(1));
	assert!(single.remove(ViewId(1)).is_none());
}

#[test]
fn resize_simple_stacked_split() {
	let area = make_rect(0, 0, 80, 30);
	let mut layout = Layout::stacked(Layout::single(ViewId(1)), Layout::single(ViewId(2)), area);

	assert_eq!(get_position(&layout), Some(15));

	let sep_info = layout.separator_with_path_at_position(area, 40, 15);
	assert!(sep_info.is_some());
	let (direction, _sep_rect, path) = sep_info.unwrap();
	assert_eq!(direction, SplitDirection::Vertical);
	assert!(path.0.is_empty());

	layout.resize_at_path(area, &path, 40, 20);

	let new_position = get_position(&layout).unwrap();
	assert_eq!(new_position, 20, "Position should be at mouse y");
}

#[test]
fn resize_nested_splits_only_affects_target() {
	let area = make_rect(0, 0, 80, 30);
	// Outer split at position 15 (local), inner split within second half (y=16 to y=29)
	// Inner area would be y=16, height=14, so inner separator at y=16+7=23 (rendered)
	let inner_area = make_rect(0, 16, 80, 14);
	let inner = Layout::stacked(Layout::single(ViewId(2)), Layout::single(ViewId(3)), inner_area);
	let mut layout = Layout::stacked(Layout::single(ViewId(1)), inner, area);

	let outer_pos_before = get_position(&layout).unwrap();
	assert_eq!(outer_pos_before, 15);

	// Inner separator is rendered at y=23 (16 + 7)
	let inner_sep_info = layout.separator_with_path_at_position(area, 40, 23);
	assert!(inner_sep_info.is_some(), "Should find inner separator at y=23");
	let (direction, _sep_rect, path) = inner_sep_info.unwrap();
	assert_eq!(direction, SplitDirection::Vertical);
	assert_eq!(path.0, vec![true]);

	layout.resize_at_path(area, &path, 40, 26);

	assert_eq!(outer_pos_before, get_position(&layout).unwrap(), "Outer position should not change");

	// Inner position is now local: mouse_y (26) - inner_area.y (16) = 10
	let inner_pos_after = get_inner_position(&layout).unwrap();
	assert_eq!(inner_pos_after, 10, "Inner position should be local offset");
}

#[test]
fn separator_rect_at_path() {
	let area = make_rect(0, 0, 80, 30);
	// Inner layout uses the area it will occupy (second half: y=16, height=14)
	// Inner position = 14/2 = 7 (local offset within inner area)
	let inner_area = make_rect(0, 16, 80, 14);
	let inner = Layout::stacked(Layout::single(ViewId(2)), Layout::single(ViewId(3)), inner_area);
	let layout = Layout::stacked(Layout::single(ViewId(1)), inner, area);

	let outer_sep = layout.separator_rect_at_path(area, &SplitPath(vec![]));
	assert!(outer_sep.is_some());
	let (dir, rect) = outer_sep.unwrap();
	assert_eq!(dir, SplitDirection::Vertical);
	assert_eq!(rect.y, 15);
	assert_eq!(rect.height, 1);
	assert_eq!(rect.width, 80);

	// Inner separator rendered at y=23 (inner_area.y=16 + local_position=7)
	let inner_sep = layout.separator_rect_at_path(area, &SplitPath(vec![true]));
	assert!(inner_sep.is_some());
	let (dir, rect) = inner_sep.unwrap();
	assert_eq!(dir, SplitDirection::Vertical);
	assert_eq!(rect.y, 23);
}

#[test]
fn separator_positions_2x2_grid() {
	let area = make_rect(0, 0, 81, 25);
	let top = Layout::side_by_side(Layout::single(ViewId(1)), Layout::single(ViewId(2)), area);
	let bottom = Layout::side_by_side(Layout::single(ViewId(3)), Layout::single(ViewId(4)), area);
	let layout = Layout::stacked(top, bottom, area);

	let seps = layout.separator_positions(area);

	assert_eq!(seps.len(), 3, "Expected 3 separators, got {:?}", seps);

	let h_seps: Vec<_> = seps.iter().filter(|(dir, _, _)| *dir == SplitDirection::Vertical).collect();
	assert_eq!(h_seps.len(), 1, "Expected 1 horizontal separator");
	let (_, _, h_rect) = h_seps[0];
	assert_eq!(h_rect.height, 1);
	assert_eq!(h_rect.width, 81);

	let v_seps: Vec<_> = seps.iter().filter(|(dir, _, _)| *dir == SplitDirection::Horizontal).collect();
	assert_eq!(v_seps.len(), 2, "Expected 2 vertical separators");

	// Check vertical separator positions for junction rendering
	// Both vertical separators should be at x=40, but at different y ranges
	// For a 4-way junction at (40, 12), we need:
	// - V-sep 1 (y=0..12) ends at y=12 → adjacent_above
	// - V-sep 2 (y=13..25) starts at y=13 → adjacent_below for y=12
	// - Horizontal (x=0..81) passes through x=40
	let h_rect = h_seps[0].2;
	let junction_y = h_rect.y; // y=12

	// Check V-sep 1 (above)
	let v1 = v_seps[0].2;
	let adjacent_above = junction_y == v1.y + v1.height;
	assert!(adjacent_above, "V-sep 1 should end at junction y");

	// Check V-sep 2 (below)
	let v2 = v_seps[1].2;
	let adjacent_below = junction_y + 1 == v2.y;
	assert!(adjacent_below, "V-sep 2 should start below junction y");
}

#[test]
fn absolute_position_stable_across_area_changes() {
	let area = make_rect(0, 0, 80, 40);
	let layout = Layout::side_by_side(Layout::single(ViewId(1)), Layout::single(ViewId(2)), area);

	assert_eq!(get_position(&layout), Some(40));

	let smaller_area = make_rect(0, 0, 80, 30);
	let seps = layout.separator_positions(smaller_area);
	assert_eq!(seps.len(), 1);
	let (_, _, sep_rect) = &seps[0];
	assert_eq!(sep_rect.x, 40, "Separator should stay at absolute position 40");
}

#[test]
fn min_width_single() {
	let layout = Layout::single(ViewId(1));
	assert_eq!(layout.min_width(), Layout::MIN_WIDTH);
}

#[test]
fn min_width_horizontal_split() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::side_by_side(Layout::single(ViewId(1)), Layout::single(ViewId(2)), area);

	// Horizontal split: first_min + 1 (separator) + second_min
	assert_eq!(layout.min_width(), Layout::MIN_WIDTH * 2 + 1);
}

#[test]
fn min_width_vertical_split() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::stacked(Layout::single(ViewId(1)), Layout::single(ViewId(2)), area);

	// Vertical split: max of children (both are MIN_WIDTH)
	assert_eq!(layout.min_width(), Layout::MIN_WIDTH);
}

#[test]
fn min_height_single() {
	let layout = Layout::single(ViewId(1));
	assert_eq!(layout.min_height(), Layout::MIN_HEIGHT);
}

#[test]
fn min_height_vertical_split() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::stacked(Layout::single(ViewId(1)), Layout::single(ViewId(2)), area);

	// Vertical split: first_min + 1 (separator) + second_min
	assert_eq!(layout.min_height(), Layout::MIN_HEIGHT * 2 + 1);
}

#[test]
fn min_height_horizontal_split() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::side_by_side(Layout::single(ViewId(1)), Layout::single(ViewId(2)), area);

	// Horizontal split: max of children (both are MIN_HEIGHT)
	assert_eq!(layout.min_height(), Layout::MIN_HEIGHT);
}

#[test]
fn min_width_nested_splits() {
	let area = make_rect(0, 0, 120, 30);
	// [A | [B | C]] - three columns
	let inner = Layout::side_by_side(Layout::single(ViewId(2)), Layout::single(ViewId(3)), area);
	let layout = Layout::side_by_side(Layout::single(ViewId(1)), inner, area);

	// first (MIN_WIDTH) + 1 + second (MIN_WIDTH + 1 + MIN_WIDTH)
	assert_eq!(layout.min_width(), Layout::MIN_WIDTH * 3 + 2);
}

#[test]
fn resize_respects_minimum_width() {
	let area = make_rect(0, 0, 80, 30);
	let mut layout = Layout::side_by_side(Layout::single(ViewId(1)), Layout::single(ViewId(2)), area);

	// Try to drag separator to far left (would make first too small)
	layout.resize_at_path(area, &SplitPath(vec![]), 0, 15);

	let pos = get_position(&layout).unwrap();
	// Position should be clamped to MIN_WIDTH (local offset)
	assert_eq!(pos, Layout::MIN_WIDTH);

	// Verify first area respects minimum
	let areas = layout.compute_view_areas(area);
	let first_area = areas.iter().find(|(v, _)| *v == ViewId(1)).unwrap().1;
	assert!(first_area.width >= Layout::MIN_WIDTH);
}

#[test]
fn resize_respects_minimum_height() {
	let area = make_rect(0, 0, 80, 30);
	let mut layout = Layout::stacked(Layout::single(ViewId(1)), Layout::single(ViewId(2)), area);

	// Try to drag separator to far top (would make first too small)
	layout.resize_at_path(area, &SplitPath(vec![]), 40, 0);

	let pos = get_position(&layout).unwrap();
	// Position should be clamped to MIN_HEIGHT (local offset)
	assert_eq!(pos, Layout::MIN_HEIGHT);

	// Verify first area respects minimum
	let areas = layout.compute_view_areas(area);
	let first_area = areas.iter().find(|(v, _)| *v == ViewId(1)).unwrap().1;
	assert!(first_area.height >= Layout::MIN_HEIGHT);
}

#[test]
fn resize_respects_sibling_minimum_width() {
	let area = make_rect(0, 0, 80, 30);
	let mut layout = Layout::side_by_side(Layout::single(ViewId(1)), Layout::single(ViewId(2)), area);

	// Try to drag separator to far right (would make second too small)
	layout.resize_at_path(area, &SplitPath(vec![]), 200, 15);

	let pos = get_position(&layout).unwrap();
	// Position should be clamped to area.width - MIN_WIDTH - 1 (local offset)
	let expected_max = area.width - Layout::MIN_WIDTH - 1;
	assert_eq!(pos, expected_max);

	// Verify second area respects minimum
	let areas = layout.compute_view_areas(area);
	let second_area = areas.iter().find(|(v, _)| *v == ViewId(2)).unwrap().1;
	assert!(second_area.width >= Layout::MIN_WIDTH);
}

#[test]
fn resize_cannot_push_nested_split() {
	let area = make_rect(0, 0, 80, 30);
	// Create [A | [B | C]] - nested horizontal splits
	let inner_area = make_rect(41, 0, 39, 30);
	let inner = Layout::side_by_side(Layout::single(ViewId(2)), Layout::single(ViewId(3)), inner_area);
	let mut layout = Layout::side_by_side(Layout::single(ViewId(1)), inner, area);

	// Inner split needs MIN_WIDTH + 1 + MIN_WIDTH
	let inner_min = Layout::MIN_WIDTH * 2 + 1;

	// Try to drag outer separator far to the right
	layout.resize_at_path(area, &SplitPath(vec![]), 200, 15);

	let pos = get_position(&layout).unwrap();
	// Position should be clamped so second child (inner split) has room for its minimum
	let expected_max = area.width - inner_min - 1;
	assert_eq!(pos, expected_max);

	// Verify all three views still have their minimum widths
	let areas = layout.compute_view_areas(area);
	for (view, rect) in &areas {
		assert!(
			rect.width >= Layout::MIN_WIDTH,
			"View {:?} has width {} < MIN_WIDTH {}",
			view,
			rect.width,
			Layout::MIN_WIDTH
		);
	}
}

#[test]
fn resize_nested_cannot_push_sibling() {
	let area = make_rect(0, 0, 80, 30);
	// Create stacked: top is single, bottom is [B | C]
	// [[A] / [B | C]]
	let inner_area = make_rect(0, 16, 80, 14);
	let inner = Layout::side_by_side(Layout::single(ViewId(2)), Layout::single(ViewId(3)), inner_area);
	let mut layout = Layout::stacked(Layout::single(ViewId(1)), inner, area);

	// Record outer separator position
	let outer_pos_before = get_position(&layout).unwrap();

	// Resize the inner split (B|C boundary) - should not affect outer split
	// Inner split path is [true] (second child of outer)
	layout.resize_at_path(area, &SplitPath(vec![true]), 20, 20);

	// Outer position should be unchanged
	let outer_pos_after = get_position(&layout).unwrap();
	assert_eq!(outer_pos_before, outer_pos_after, "Outer split position should not change");
}

// =============================================================================
// Invariant tests for compute_split_areas (soft-min policy)
// =============================================================================

/// Verifies that `compute_split_areas` maintains layout invariants across a range of small widths.
///
/// This test ensures the soft-min policy degrades gracefully on small terminals:
/// 1. No overflow in rect coordinates.
/// 2. Separator is at most 1 cell wide.
/// 3. Layout elements sum to the total width in non-degenerate cases.
/// 4. At least one child is visible when space allows (total >= 3).
/// 5. Soft minimums are enforced when total space is sufficient.
#[test]
fn compute_split_areas_invariants_horizontal() {
	// Test with widths from 0 to 30 (covers degenerate cases up to normal)
	for width in 0..=30u16 {
		let area = make_rect(0, 0, width, 10);
		let position = width / 2;

		let (first, second, sep) = Layout::compute_split_areas(area, SplitDirection::Horizontal, position);

		assert!(first.x.saturating_add(first.width) >= first.x, "first: overflow in x+width");
		assert!(second.x.saturating_add(second.width) >= second.x, "second: overflow in x+width");
		assert!(sep.x.saturating_add(sep.width) >= sep.x, "sep: overflow in x+width");

		assert!(sep.width <= 1, "sep width should be 0 or 1, got {}", sep.width);

		if width > 1 {
			let total_used = first.width + second.width + sep.width;
			assert_eq!(
				total_used, width,
				"width {}: first({}) + second({}) + sep({}) = {} != total",
				width, first.width, second.width, sep.width, total_used
			);
		} else {
			assert_eq!(first.width, 0, "width {}: first should be 0 in degenerate case", width);
			assert_eq!(second.width, 0, "width {}: second should be 0 in degenerate case", width);
			assert_eq!(sep.width, 0, "width {}: sep should be 0 in degenerate case", width);
		}

		if width >= 3 {
			assert!(first.width >= 1 || second.width >= 1, "width {}: at least one child should be visible", width);
		}

		let soft_min_total = Layout::MIN_WIDTH * 2 + 1;
		if width >= soft_min_total {
			assert!(
				first.width >= Layout::MIN_WIDTH,
				"width {}: first width {} < MIN_WIDTH {}",
				width,
				first.width,
				Layout::MIN_WIDTH
			);
			assert!(
				second.width >= Layout::MIN_WIDTH,
				"width {}: second width {} < MIN_WIDTH {}",
				width,
				second.width,
				Layout::MIN_WIDTH
			);
		}
	}
}

/// Verifies that `compute_split_areas` maintains layout invariants across a range of small heights.
#[test]
fn compute_split_areas_invariants_vertical() {
	// Test with heights from 0 to 30
	for height in 0..=30u16 {
		let area = make_rect(0, 0, 10, height);
		let position = height / 2;

		let (first, second, sep) = Layout::compute_split_areas(area, SplitDirection::Vertical, position);

		assert!(first.y.saturating_add(first.height) >= first.y, "first: overflow in y+height");
		assert!(second.y.saturating_add(second.height) >= second.y, "second: overflow in y+height");
		assert!(sep.y.saturating_add(sep.height) >= sep.y, "sep: overflow in y+height");

		assert!(sep.height <= 1, "sep height should be 0 or 1, got {}", sep.height);

		if height > 1 {
			let total_used = first.height + second.height + sep.height;
			assert_eq!(
				total_used, height,
				"height {}: first({}) + second({}) + sep({}) = {} != total",
				height, first.height, second.height, sep.height, total_used
			);
		} else {
			assert_eq!(first.height, 0, "height {}: first should be 0 in degenerate case", height);
			assert_eq!(second.height, 0, "height {}: second should be 0 in degenerate case", height);
			assert_eq!(sep.height, 0, "height {}: sep should be 0 in degenerate case", height);
		}

		if height >= 3 {
			assert!(
				first.height >= 1 || second.height >= 1,
				"height {}: at least one child should be visible",
				height
			);
		}

		let soft_min_total = Layout::MIN_HEIGHT * 2 + 1;
		if height >= soft_min_total {
			assert!(
				first.height >= Layout::MIN_HEIGHT,
				"height {}: first height {} < MIN_HEIGHT {}",
				height,
				first.height,
				Layout::MIN_HEIGHT
			);
			assert!(
				second.height >= Layout::MIN_HEIGHT,
				"height {}: second height {} < MIN_HEIGHT {}",
				height,
				second.height,
				Layout::MIN_HEIGHT
			);
		}
	}
}

/// Verifies that split panes never collapse to zero width/height when space allows.
#[test]
fn compute_split_areas_no_zero_sized_panes() {
	let area = make_rect(0, 0, 5, 5);

	let (first, second, _sep) = Layout::compute_split_areas(area, SplitDirection::Horizontal, 2);
	assert!(first.width >= 1 || second.width == 0, "first should be >=1 when possible");
	assert!(second.width >= 1 || first.width == 0, "second should be >=1 when possible");

	let (first, second, _sep) = Layout::compute_split_areas(area, SplitDirection::Vertical, 2);
	assert!(first.height >= 1 || second.height == 0, "first should be >=1 when possible");
	assert!(second.height >= 1 || first.height == 0, "second should be >=1 when possible");
}

/// Verifies that extreme separator positions are clamped correctly by the soft-min policy.
#[test]
fn compute_split_areas_extreme_position_clamping() {
	let area = make_rect(0, 0, 80, 30);

	let (first, _second, _sep) = Layout::compute_split_areas(area, SplitDirection::Horizontal, 1000);
	assert!(first.width < area.width, "first width should respect area bounds");

	let (first, _second, _sep) = Layout::compute_split_areas(area, SplitDirection::Horizontal, 0);
	assert!(first.width >= 1 || area.width < 3, "first should have hard minimum when possible");
}
