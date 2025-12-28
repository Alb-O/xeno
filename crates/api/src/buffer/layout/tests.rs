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
