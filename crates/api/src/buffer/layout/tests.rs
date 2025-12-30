use super::*;

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
	let layout = Layout::side_by_side(Layout::single(BufferId(1)), Layout::single(BufferId(2)));

	assert_eq!(layout.first_buffer(), Some(BufferId(1)));
	assert_eq!(layout.buffer_ids(), vec![BufferId(1), BufferId(2)]);
	assert!(layout.contains(BufferId(1)));
	assert!(layout.contains(BufferId(2)));
	assert!(!layout.contains(BufferId(3)));
}

#[test]
fn next_prev_buffer() {
	let layout = Layout::side_by_side(Layout::single(BufferId(1)), Layout::single(BufferId(2)));

	assert_eq!(layout.next_buffer(BufferId(1)), BufferId(2));
	assert_eq!(layout.next_buffer(BufferId(2)), BufferId(1));
	assert_eq!(layout.prev_buffer(BufferId(1)), BufferId(2));
	assert_eq!(layout.prev_buffer(BufferId(2)), BufferId(1));
}

#[test]
fn remove_buffer() {
	let layout = Layout::side_by_side(Layout::single(BufferId(1)), Layout::single(BufferId(2)));

	let after_remove = layout.remove(BufferId(1)).unwrap();
	assert_eq!(after_remove.buffer_ids(), vec![BufferId(2)]);

	// Removing the only buffer returns None
	let single = Layout::single(BufferId(1));
	assert!(single.remove(BufferId(1)).is_none());
}

fn make_rect(x: u16, y: u16, width: u16, height: u16) -> evildoer_tui::layout::Rect {
	evildoer_tui::layout::Rect {
		x,
		y,
		width,
		height,
	}
}

fn get_ratio(layout: &Layout) -> Option<f32> {
	match layout {
		Layout::Split { ratio, .. } => Some(*ratio),
		Layout::Single(_) => None,
	}
}

fn get_inner_ratio(layout: &Layout) -> Option<f32> {
	match layout {
		Layout::Split { second, .. } => get_ratio(second),
		Layout::Single(_) => None,
	}
}

#[test]
fn resize_simple_stacked_split() {
	let mut layout = Layout::stacked(Layout::single(BufferId(1)), Layout::single(BufferId(2)));
	let area = make_rect(0, 0, 80, 30);

	assert_eq!(get_ratio(&layout), Some(0.5));

	let sep_info = layout.separator_with_path_at_position(area, 40, 15);
	assert!(sep_info.is_some());
	let (direction, _sep_rect, path) = sep_info.unwrap();
	assert_eq!(direction, SplitDirection::Vertical);
	assert!(path.0.is_empty());

	layout.resize_at_path(area, &path, 40, 20);

	let new_ratio = get_ratio(&layout).unwrap();
	assert!(new_ratio > 0.5, "Ratio should increase: {}", new_ratio);
	assert!(
		(new_ratio - 0.67).abs() < 0.05,
		"Ratio should be ~0.67: {}",
		new_ratio
	);
}

#[test]
fn resize_nested_splits_only_affects_target() {
	// Create A over (B over C)
	let inner = Layout::stacked(Layout::single(BufferId(2)), Layout::single(BufferId(3)));
	let mut layout = Layout::stacked(Layout::single(BufferId(1)), inner);
	let area = make_rect(0, 0, 80, 30);

	let outer_ratio_before = get_ratio(&layout).unwrap();
	let inner_ratio_before = get_inner_ratio(&layout).unwrap();
	assert_eq!(outer_ratio_before, 0.5);
	assert_eq!(inner_ratio_before, 0.5);

	// Find the INNER separator (between B and C) around y=23
	let inner_sep_info = layout.separator_with_path_at_position(area, 40, 23);
	assert!(
		inner_sep_info.is_some(),
		"Should find inner separator at y=23"
	);
	let (direction, _sep_rect, path) = inner_sep_info.unwrap();
	assert_eq!(direction, SplitDirection::Vertical);
	assert_eq!(path.0, vec![true]);

	layout.resize_at_path(area, &path, 40, 26);

	// Outer ratio UNCHANGED
	assert_eq!(
		outer_ratio_before,
		get_ratio(&layout).unwrap(),
		"Outer ratio should not change"
	);

	// Inner ratio changed
	let inner_ratio_after = get_inner_ratio(&layout).unwrap();
	assert!(
		inner_ratio_after > inner_ratio_before,
		"Inner ratio should increase"
	);
}

#[test]
fn resize_outer_split_preserves_inner_absolute_position() {
	// Create A over (B over C)
	let inner = Layout::stacked(Layout::single(BufferId(2)), Layout::single(BufferId(3)));
	let mut layout = Layout::stacked(Layout::single(BufferId(1)), inner);
	let area = make_rect(0, 0, 80, 30);

	// Find the OUTER separator
	let outer_sep_info = layout.separator_with_path_at_position(area, 40, 15);
	assert!(outer_sep_info.is_some());
	let (direction, _sep_rect, path) = outer_sep_info.unwrap();
	assert_eq!(direction, SplitDirection::Vertical);
	assert!(path.0.is_empty());

	// Drag the OUTER separator - inner ratio should adjust to preserve absolute position
	layout.resize_at_path(area, &path, 40, 10);

	// The inner ratio will have changed to compensate for the area change
	// This is the key behavior: absolute positions are preserved
	let inner_ratio = get_inner_ratio(&layout).unwrap();
	assert!(
		inner_ratio != 0.5,
		"Inner ratio should adjust: {}",
		inner_ratio
	);
}

#[test]
fn separator_rect_at_path() {
	let inner = Layout::stacked(Layout::single(BufferId(2)), Layout::single(BufferId(3)));
	let layout = Layout::stacked(Layout::single(BufferId(1)), inner);
	let area = make_rect(0, 0, 80, 30);

	// Outer separator (empty path)
	let outer_sep = layout.separator_rect_at_path(area, &SplitPath(vec![]));
	assert!(outer_sep.is_some());
	let (dir, rect) = outer_sep.unwrap();
	assert_eq!(dir, SplitDirection::Vertical);
	assert_eq!(rect.y, 15);
	assert_eq!(rect.height, 1);
	assert_eq!(rect.width, 80);

	// Inner separator (path = [true])
	let inner_sep = layout.separator_rect_at_path(area, &SplitPath(vec![true]));
	assert!(inner_sep.is_some());
	let (dir, rect) = inner_sep.unwrap();
	assert_eq!(dir, SplitDirection::Vertical);
	assert_eq!(rect.y, 23);
}

#[test]
fn separator_positions_2x2_grid() {
	// 2x2 grid: (A|B) over (C|D)
	let top = Layout::side_by_side(Layout::single(BufferId(1)), Layout::single(BufferId(2)));
	let bottom = Layout::side_by_side(Layout::single(BufferId(3)), Layout::single(BufferId(4)));
	let layout = Layout::stacked(top, bottom);
	let area = make_rect(0, 0, 81, 25); // odd width/height for clean splits

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
	assert_eq!(h_rect.y, 13);

	let v_seps: Vec<_> = seps
		.iter()
		.filter(|(dir, _, _)| *dir == SplitDirection::Horizontal)
		.collect();
	assert_eq!(v_seps.len(), 2, "Expected 2 vertical separators");

	for (_, _, v_rect) in &v_seps {
		assert_eq!(v_rect.width, 1);
		assert_eq!(v_rect.x, 41, "Vertical separator at wrong x: {:?}", v_rect);
	}

	let top_v = v_seps.iter().find(|(_, _, r)| r.y < 13);
	let bottom_v = v_seps.iter().find(|(_, _, r)| r.y > 13);
	assert!(top_v.is_some(), "Expected vertical separator in top half");
	assert!(
		bottom_v.is_some(),
		"Expected vertical separator in bottom half"
	);
}

#[test]
fn separator_junction_detection() {
	// 2x2 grid: (A|B) over (C|D)
	// The horizontal line at y=13 should intersect with vertical lines at x=41
	let top = Layout::side_by_side(Layout::single(BufferId(1)), Layout::single(BufferId(2)));
	let bottom = Layout::side_by_side(Layout::single(BufferId(3)), Layout::single(BufferId(4)));
	let layout = Layout::stacked(top, bottom);
	let area = make_rect(0, 0, 81, 25);

	let seps = layout.separator_positions(area);

	let h_sep = seps
		.iter()
		.find(|(dir, _, _)| *dir == SplitDirection::Vertical)
		.expect("horizontal separator");

	let v_seps: Vec<_> = seps
		.iter()
		.filter(|(dir, _, _)| *dir == SplitDirection::Horizontal)
		.collect();
	let (_, _, h_rect) = h_sep;
	assert_eq!(h_rect.y, 13);
	assert_eq!(h_rect.x, 0);
	assert_eq!(h_rect.width, 81);

	let top_v = v_seps
		.iter()
		.find(|(_, _, r)| r.y == 0)
		.expect("top vertical");
	let bottom_v = v_seps
		.iter()
		.find(|(_, _, r)| r.y > 13)
		.expect("bottom vertical");

	let (_, _, top_v_rect) = top_v;
	let (_, _, bottom_v_rect) = bottom_v;

	assert_eq!(top_v_rect.x, 41);
	assert_eq!(top_v_rect.y, 0);
	assert_eq!(top_v_rect.height, 13);

	assert_eq!(bottom_v_rect.x, 41);
	assert_eq!(bottom_v_rect.y, 14);

	// Verify separators don't overlap the horizontal separator row
	assert!(
		top_v_rect.bottom() <= h_rect.y,
		"Top vertical should end before horizontal: {} <= {}",
		top_v_rect.bottom(),
		h_rect.y
	);
	assert!(
		bottom_v_rect.y > h_rect.y,
		"Bottom vertical should start after horizontal: {} > {}",
		bottom_v_rect.y,
		h_rect.y
	);
}
