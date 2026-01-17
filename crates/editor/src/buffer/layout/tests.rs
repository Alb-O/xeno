use super::*;

fn make_rect(x: u16, y: u16, width: u16, height: u16) -> xeno_tui::layout::Rect {
	xeno_tui::layout::Rect {
		x,
		y,
		width,
		height,
	}
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
	let layout = Layout::single(BufferId(1));
	assert_eq!(layout.first_buffer(), Some(BufferId(1)));
	assert_eq!(layout.buffer_ids(), vec![BufferId(1)]);
	assert!(layout.contains(BufferId(1)));
	assert!(!layout.contains(BufferId(2)));
}

#[test]
fn side_by_side_split() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::side_by_side(
		Layout::single(BufferId(1)),
		Layout::single(BufferId(2)),
		area,
	);

	assert_eq!(layout.first_buffer(), Some(BufferId(1)));
	assert_eq!(layout.buffer_ids(), vec![BufferId(1), BufferId(2)]);
	assert!(layout.contains(BufferId(1)));
	assert!(layout.contains(BufferId(2)));
	assert!(!layout.contains(BufferId(3)));
	assert_eq!(get_position(&layout), Some(40));
}

#[test]
fn next_prev_buffer() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::side_by_side(
		Layout::single(BufferId(1)),
		Layout::single(BufferId(2)),
		area,
	);

	assert_eq!(layout.next_buffer(BufferId(1)), BufferId(2));
	assert_eq!(layout.next_buffer(BufferId(2)), BufferId(1));
	assert_eq!(layout.prev_buffer(BufferId(1)), BufferId(2));
	assert_eq!(layout.prev_buffer(BufferId(2)), BufferId(1));
}

#[test]
fn remove_buffer() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::side_by_side(
		Layout::single(BufferId(1)),
		Layout::single(BufferId(2)),
		area,
	);

	let after_remove = layout.remove(BufferId(1)).unwrap();
	assert_eq!(after_remove.buffer_ids(), vec![BufferId(2)]);

	let single = Layout::single(BufferId(1));
	assert!(single.remove(BufferId(1)).is_none());
}

#[test]
fn resize_simple_stacked_split() {
	let area = make_rect(0, 0, 80, 30);
	let mut layout = Layout::stacked(
		Layout::single(BufferId(1)),
		Layout::single(BufferId(2)),
		area,
	);

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
	let inner = Layout::stacked(
		Layout::single(BufferId(2)),
		Layout::single(BufferId(3)),
		inner_area,
	);
	let mut layout = Layout::stacked(Layout::single(BufferId(1)), inner, area);

	let outer_pos_before = get_position(&layout).unwrap();
	assert_eq!(outer_pos_before, 15);

	// Inner separator is rendered at y=23 (16 + 7)
	let inner_sep_info = layout.separator_with_path_at_position(area, 40, 23);
	assert!(
		inner_sep_info.is_some(),
		"Should find inner separator at y=23"
	);
	let (direction, _sep_rect, path) = inner_sep_info.unwrap();
	assert_eq!(direction, SplitDirection::Vertical);
	assert_eq!(path.0, vec![true]);

	layout.resize_at_path(area, &path, 40, 26);

	assert_eq!(
		outer_pos_before,
		get_position(&layout).unwrap(),
		"Outer position should not change"
	);

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
	let inner = Layout::stacked(
		Layout::single(BufferId(2)),
		Layout::single(BufferId(3)),
		inner_area,
	);
	let layout = Layout::stacked(Layout::single(BufferId(1)), inner, area);

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
	let top = Layout::side_by_side(
		Layout::single(BufferId(1)),
		Layout::single(BufferId(2)),
		area,
	);
	let bottom = Layout::side_by_side(
		Layout::single(BufferId(3)),
		Layout::single(BufferId(4)),
		area,
	);
	let layout = Layout::stacked(top, bottom, area);

	let seps = layout.separator_positions(area);

	assert_eq!(seps.len(), 3, "Expected 3 separators, got {:?}", seps);

	let h_seps: Vec<_> = seps
		.iter()
		.filter(|(dir, _, _)| *dir == SplitDirection::Vertical)
		.collect();
	assert_eq!(h_seps.len(), 1, "Expected 1 horizontal separator");
	let (_, _, h_rect) = h_seps[0];
	assert_eq!(h_rect.height, 1);
	assert_eq!(h_rect.width, 81);

	let v_seps: Vec<_> = seps
		.iter()
		.filter(|(dir, _, _)| *dir == SplitDirection::Horizontal)
		.collect();
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
	let layout = Layout::side_by_side(
		Layout::single(BufferId(1)),
		Layout::single(BufferId(2)),
		area,
	);

	assert_eq!(get_position(&layout), Some(40));

	let smaller_area = make_rect(0, 0, 80, 30);
	let seps = layout.separator_positions(smaller_area);
	assert_eq!(seps.len(), 1);
	let (_, _, sep_rect) = &seps[0];
	assert_eq!(
		sep_rect.x, 40,
		"Separator should stay at absolute position 40"
	);
}

#[test]
fn min_width_single() {
	let layout = Layout::single(BufferId(1));
	assert_eq!(layout.min_width(), Layout::MIN_WIDTH);
}

#[test]
fn min_width_horizontal_split() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::side_by_side(
		Layout::single(BufferId(1)),
		Layout::single(BufferId(2)),
		area,
	);

	// Horizontal split: first_min + 1 (separator) + second_min
	assert_eq!(layout.min_width(), Layout::MIN_WIDTH * 2 + 1);
}

#[test]
fn min_width_vertical_split() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::stacked(
		Layout::single(BufferId(1)),
		Layout::single(BufferId(2)),
		area,
	);

	// Vertical split: max of children (both are MIN_WIDTH)
	assert_eq!(layout.min_width(), Layout::MIN_WIDTH);
}

#[test]
fn min_height_single() {
	let layout = Layout::single(BufferId(1));
	assert_eq!(layout.min_height(), Layout::MIN_HEIGHT);
}

#[test]
fn min_height_vertical_split() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::stacked(
		Layout::single(BufferId(1)),
		Layout::single(BufferId(2)),
		area,
	);

	// Vertical split: first_min + 1 (separator) + second_min
	assert_eq!(layout.min_height(), Layout::MIN_HEIGHT * 2 + 1);
}

#[test]
fn min_height_horizontal_split() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::side_by_side(
		Layout::single(BufferId(1)),
		Layout::single(BufferId(2)),
		area,
	);

	// Horizontal split: max of children (both are MIN_HEIGHT)
	assert_eq!(layout.min_height(), Layout::MIN_HEIGHT);
}

#[test]
fn min_width_nested_splits() {
	let area = make_rect(0, 0, 120, 30);
	// [A | [B | C]] - three columns
	let inner = Layout::side_by_side(
		Layout::single(BufferId(2)),
		Layout::single(BufferId(3)),
		area,
	);
	let layout = Layout::side_by_side(Layout::single(BufferId(1)), inner, area);

	// first (MIN_WIDTH) + 1 + second (MIN_WIDTH + 1 + MIN_WIDTH)
	assert_eq!(layout.min_width(), Layout::MIN_WIDTH * 3 + 2);
}

#[test]
fn resize_respects_minimum_width() {
	let area = make_rect(0, 0, 80, 30);
	let mut layout = Layout::side_by_side(
		Layout::single(BufferId(1)),
		Layout::single(BufferId(2)),
		area,
	);

	// Try to drag separator to far left (would make first too small)
	layout.resize_at_path(area, &SplitPath(vec![]), 0, 15);

	let pos = get_position(&layout).unwrap();
	// Position should be clamped to MIN_WIDTH (local offset)
	assert_eq!(pos, Layout::MIN_WIDTH);

	// Verify first area respects minimum
	let areas = layout.compute_view_areas(area);
	let first_area = areas.iter().find(|(v, _)| *v == BufferId(1)).unwrap().1;
	assert!(first_area.width >= Layout::MIN_WIDTH);
}

#[test]
fn resize_respects_minimum_height() {
	let area = make_rect(0, 0, 80, 30);
	let mut layout = Layout::stacked(
		Layout::single(BufferId(1)),
		Layout::single(BufferId(2)),
		area,
	);

	// Try to drag separator to far top (would make first too small)
	layout.resize_at_path(area, &SplitPath(vec![]), 40, 0);

	let pos = get_position(&layout).unwrap();
	// Position should be clamped to MIN_HEIGHT (local offset)
	assert_eq!(pos, Layout::MIN_HEIGHT);

	// Verify first area respects minimum
	let areas = layout.compute_view_areas(area);
	let first_area = areas.iter().find(|(v, _)| *v == BufferId(1)).unwrap().1;
	assert!(first_area.height >= Layout::MIN_HEIGHT);
}

#[test]
fn resize_respects_sibling_minimum_width() {
	let area = make_rect(0, 0, 80, 30);
	let mut layout = Layout::side_by_side(
		Layout::single(BufferId(1)),
		Layout::single(BufferId(2)),
		area,
	);

	// Try to drag separator to far right (would make second too small)
	layout.resize_at_path(area, &SplitPath(vec![]), 200, 15);

	let pos = get_position(&layout).unwrap();
	// Position should be clamped to area.width - MIN_WIDTH - 1 (local offset)
	let expected_max = area.width - Layout::MIN_WIDTH - 1;
	assert_eq!(pos, expected_max);

	// Verify second area respects minimum
	let areas = layout.compute_view_areas(area);
	let second_area = areas.iter().find(|(v, _)| *v == BufferId(2)).unwrap().1;
	assert!(second_area.width >= Layout::MIN_WIDTH);
}

#[test]
fn resize_cannot_push_nested_split() {
	let area = make_rect(0, 0, 80, 30);
	// Create [A | [B | C]] - nested horizontal splits
	let inner_area = make_rect(41, 0, 39, 30);
	let inner = Layout::side_by_side(
		Layout::single(BufferId(2)),
		Layout::single(BufferId(3)),
		inner_area,
	);
	let mut layout = Layout::side_by_side(Layout::single(BufferId(1)), inner, area);

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
	let inner = Layout::side_by_side(
		Layout::single(BufferId(2)),
		Layout::single(BufferId(3)),
		inner_area,
	);
	let mut layout = Layout::stacked(Layout::single(BufferId(1)), inner, area);

	// Record outer separator position
	let outer_pos_before = get_position(&layout).unwrap();

	// Resize the inner split (B|C boundary) - should not affect outer split
	// Inner split path is [true] (second child of outer)
	layout.resize_at_path(area, &SplitPath(vec![true]), 20, 20);

	// Outer position should be unchanged
	let outer_pos_after = get_position(&layout).unwrap();
	assert_eq!(
		outer_pos_before, outer_pos_after,
		"Outer split position should not change"
	);
}
