use super::*;

fn make_rect(x: u16, y: u16, width: u16, height: u16) -> evildoer_tui::layout::Rect {
	evildoer_tui::layout::Rect {
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
	let layout = Layout::side_by_side(Layout::single(BufferId(1)), Layout::single(BufferId(2)), area);

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
	let layout = Layout::side_by_side(Layout::single(BufferId(1)), Layout::single(BufferId(2)), area);

	assert_eq!(layout.next_buffer(BufferId(1)), BufferId(2));
	assert_eq!(layout.next_buffer(BufferId(2)), BufferId(1));
	assert_eq!(layout.prev_buffer(BufferId(1)), BufferId(2));
	assert_eq!(layout.prev_buffer(BufferId(2)), BufferId(1));
}

#[test]
fn remove_buffer() {
	let area = make_rect(0, 0, 80, 30);
	let layout = Layout::side_by_side(Layout::single(BufferId(1)), Layout::single(BufferId(2)), area);

	let after_remove = layout.remove(BufferId(1)).unwrap();
	assert_eq!(after_remove.buffer_ids(), vec![BufferId(2)]);

	let single = Layout::single(BufferId(1));
	assert!(single.remove(BufferId(1)).is_none());
}

#[test]
fn resize_simple_stacked_split() {
	let area = make_rect(0, 0, 80, 30);
	let mut layout = Layout::stacked(Layout::single(BufferId(1)), Layout::single(BufferId(2)), area);

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
	// Outer split at y=15, inner split within second half (y=16 to y=29)
	// Inner area would be y=16, height=14, so inner separator at y=16+7=23
	let inner_area = make_rect(0, 16, 80, 14);
	let inner = Layout::stacked(Layout::single(BufferId(2)), Layout::single(BufferId(3)), inner_area);
	let mut layout = Layout::stacked(Layout::single(BufferId(1)), inner, area);

	let outer_pos_before = get_position(&layout).unwrap();
	assert_eq!(outer_pos_before, 15);

	// Inner separator is at y=23 (16 + 7)
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

	let inner_pos_after = get_inner_position(&layout).unwrap();
	assert_eq!(inner_pos_after, 26, "Inner position should be at mouse y");
}

#[test]
fn separator_rect_at_path() {
	let area = make_rect(0, 0, 80, 30);
	// Inner layout uses the area it will occupy (second half: y=16, height=14)
	let inner_area = make_rect(0, 16, 80, 14);
	let inner = Layout::stacked(Layout::single(BufferId(2)), Layout::single(BufferId(3)), inner_area);
	let layout = Layout::stacked(Layout::single(BufferId(1)), inner, area);

	let outer_sep = layout.separator_rect_at_path(area, &SplitPath(vec![]));
	assert!(outer_sep.is_some());
	let (dir, rect) = outer_sep.unwrap();
	assert_eq!(dir, SplitDirection::Vertical);
	assert_eq!(rect.y, 15);
	assert_eq!(rect.height, 1);
	assert_eq!(rect.width, 80);

	// Inner separator at y=23 (16 + 7)
	let inner_sep = layout.separator_rect_at_path(area, &SplitPath(vec![true]));
	assert!(inner_sep.is_some());
	let (dir, rect) = inner_sep.unwrap();
	assert_eq!(dir, SplitDirection::Vertical);
	assert_eq!(rect.y, 23);
}

#[test]
fn separator_positions_2x2_grid() {
	let area = make_rect(0, 0, 81, 25);
	let top = Layout::side_by_side(Layout::single(BufferId(1)), Layout::single(BufferId(2)), area);
	let bottom = Layout::side_by_side(Layout::single(BufferId(3)), Layout::single(BufferId(4)), area);
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
}

#[test]
fn absolute_position_stable_across_area_changes() {
	let area = make_rect(0, 0, 80, 40);
	let layout = Layout::side_by_side(Layout::single(BufferId(1)), Layout::single(BufferId(2)), area);

	assert_eq!(get_position(&layout), Some(40));

	let smaller_area = make_rect(0, 0, 80, 30);
	let seps = layout.separator_positions(smaller_area);
	assert_eq!(seps.len(), 1);
	let (_, _, sep_rect) = &seps[0];
	assert_eq!(sep_rect.x, 40, "Separator should stay at absolute position 40");
}
