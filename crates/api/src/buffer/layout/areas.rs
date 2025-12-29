//! Area computation and separator handling for layout splits.

use evildoer_tui::layout::Rect;

use super::Layout;
use super::types::{BufferView, SplitDirection, SplitPath};
use crate::buffer::BufferId;

impl Layout {
	/// Finds the view at the given screen coordinates.
	pub fn view_at_position(&self, area: Rect, x: u16, y: u16) -> Option<(BufferView, Rect)> {
		self.compute_view_areas(area).into_iter().find(|(_, rect)| {
			x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
		})
	}

	/// Computes rectangular areas for each view in the layout.
	pub fn compute_view_areas(&self, area: Rect) -> Vec<(BufferView, Rect)> {
		match self {
			Layout::Single(view) => vec![(*view, area)],
			Layout::Split {
				direction,
				ratio,
				first,
				second,
			} => {
				let (first_area, second_area) = Self::split_area(area, *direction, *ratio);
				let mut areas = first.compute_view_areas(first_area);
				areas.extend(second.compute_view_areas(second_area));
				areas
			}
		}
	}

	/// Computes rectangular areas for each buffer in the layout.
	pub fn compute_areas(&self, area: Rect) -> Vec<(BufferId, Rect)> {
		self.compute_view_areas(area)
			.into_iter()
			.filter_map(|(view, rect)| view.as_text().map(|id| (id, rect)))
			.collect()
	}

	/// Helper to split an area according to direction and ratio.
	fn split_area(area: Rect, direction: SplitDirection, ratio: f32) -> (Rect, Rect) {
		let (first, second, _) = Self::compute_split_areas(area, direction, ratio);
		(first, second)
	}

	/// Finds the separator at the given screen coordinates.
	pub fn separator_at_position(
		&self,
		area: Rect,
		x: u16,
		y: u16,
	) -> Option<(SplitDirection, Rect)> {
		self.separator_positions(area)
			.into_iter()
			.find(|(_, _, rect)| {
				x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
			})
			.map(|(dir, _, rect)| (dir, rect))
	}

	/// Finds the separator and its path at the given screen coordinates.
	pub fn separator_with_path_at_position(
		&self,
		area: Rect,
		x: u16,
		y: u16,
	) -> Option<(SplitDirection, Rect, SplitPath)> {
		self.find_separator_with_path(area, x, y, SplitPath::default())
	}

	fn find_separator_with_path(
		&self,
		area: Rect,
		x: u16,
		y: u16,
		current_path: SplitPath,
	) -> Option<(SplitDirection, Rect, SplitPath)> {
		let Layout::Split {
			direction,
			ratio,
			first,
			second,
		} = self
		else {
			return None;
		};

		let (first_area, second_area, sep_rect) =
			Self::compute_split_areas(area, *direction, *ratio);

		// Check if point is on this separator
		if x >= sep_rect.x
			&& x < sep_rect.x + sep_rect.width
			&& y >= sep_rect.y
			&& y < sep_rect.y + sep_rect.height
		{
			return Some((*direction, sep_rect, current_path));
		}

		// Recurse into first child
		let mut first_path = current_path.clone();
		first_path.0.push(false);
		if let Some(result) = first.find_separator_with_path(first_area, x, y, first_path) {
			return Some(result);
		}

		// Recurse into second child
		let mut second_path = current_path;
		second_path.0.push(true);
		second.find_separator_with_path(second_area, x, y, second_path)
	}

	/// Resizes the split at the given path based on mouse position.
	/// Child splits have their ratios adjusted to keep separators at same absolute positions.
	pub fn resize_at_path(
		&mut self,
		area: Rect,
		path: &SplitPath,
		mouse_x: u16,
		mouse_y: u16,
	) -> bool {
		self.do_resize_at_path(area, &path.0, mouse_x, mouse_y)
	}

	fn do_resize_at_path(&mut self, area: Rect, path: &[bool], mouse_x: u16, mouse_y: u16) -> bool {
		let Layout::Split {
			direction,
			ratio,
			first,
			second,
		} = self
		else {
			return false;
		};

		if path.is_empty() {
			// This is the target split - calculate new ratio
			let new_ratio = match direction {
				SplitDirection::Horizontal => {
					let relative_x = mouse_x.saturating_sub(area.x);
					relative_x.clamp(1, area.width.saturating_sub(2)) as f32 / area.width as f32
				}
				SplitDirection::Vertical => {
					let relative_y = mouse_y.saturating_sub(area.y);
					relative_y.clamp(1, area.height.saturating_sub(2)) as f32 / area.height as f32
				}
			}
			.clamp(0.1, 0.9);

			// Collect child separator positions before resize
			let (old_first_area, old_second_area, _) =
				Self::compute_split_areas(area, *direction, *ratio);
			let first_positions = first.collect_separator_positions(old_first_area);
			let second_positions = second.collect_separator_positions(old_second_area);

			*ratio = new_ratio;

			// Adjust child ratios to preserve absolute separator positions
			let (new_first_area, new_second_area, _) =
				Self::compute_split_areas(area, *direction, new_ratio);
			first.adjust_ratios_for_new_area(old_first_area, new_first_area, &first_positions);
			second.adjust_ratios_for_new_area(old_second_area, new_second_area, &second_positions);

			return true;
		}

		// Follow the path
		let (first_area, second_area, _) = Self::compute_split_areas(area, *direction, *ratio);
		if path[0] {
			second.do_resize_at_path(second_area, &path[1..], mouse_x, mouse_y)
		} else {
			first.do_resize_at_path(first_area, &path[1..], mouse_x, mouse_y)
		}
	}

	pub(super) fn collect_separator_positions(&self, area: Rect) -> Vec<(SplitDirection, u16)> {
		let Layout::Split {
			direction,
			ratio,
			first,
			second,
		} = self
		else {
			return vec![];
		};

		let (first_area, second_area, sep_rect) =
			Self::compute_split_areas(area, *direction, *ratio);

		let sep_pos = match direction {
			SplitDirection::Horizontal => sep_rect.x,
			SplitDirection::Vertical => sep_rect.y,
		};

		let mut positions = vec![(*direction, sep_pos)];
		positions.extend(first.collect_separator_positions(first_area));
		positions.extend(second.collect_separator_positions(second_area));
		positions
	}

	pub(super) fn adjust_ratios_for_new_area(
		&mut self,
		old_area: Rect,
		new_area: Rect,
		old_positions: &[(SplitDirection, u16)],
	) {
		if old_positions.is_empty() {
			return;
		}

		let Layout::Split {
			direction,
			ratio,
			first,
			second,
		} = self
		else {
			return;
		};

		let Some(&(_, old_pos)) = old_positions.first() else {
			return;
		};

		// Calculate new ratio to keep separator at same absolute position
		let new_ratio = match direction {
			SplitDirection::Horizontal if new_area.width > 1 => {
				(old_pos.saturating_sub(new_area.x) as f32 / new_area.width as f32).clamp(0.1, 0.9)
			}
			SplitDirection::Vertical if new_area.height > 1 => {
				(old_pos.saturating_sub(new_area.y) as f32 / new_area.height as f32).clamp(0.1, 0.9)
			}
			_ => *ratio,
		};

		let (old_first_area, old_second_area, _) =
			Self::compute_split_areas(old_area, *direction, *ratio);
		*ratio = new_ratio;
		let (new_first_area, new_second_area, _) =
			Self::compute_split_areas(new_area, *direction, new_ratio);

		// Recursively adjust children
		let remaining = &old_positions[1..];
		let first_count = first.separator_count();
		let (first_positions, second_positions) =
			remaining.split_at(first_count.min(remaining.len()));

		first.adjust_ratios_for_new_area(old_first_area, new_first_area, first_positions);
		second.adjust_ratios_for_new_area(old_second_area, new_second_area, second_positions);
	}

	pub(super) fn separator_count(&self) -> usize {
		match self {
			Layout::Single(_) => 0,
			Layout::Split { first, second, .. } => {
				1 + first.separator_count() + second.separator_count()
			}
		}
	}

	/// Gets the separator rect for a split at the given path.
	pub fn separator_rect_at_path(
		&self,
		area: Rect,
		path: &SplitPath,
	) -> Option<(SplitDirection, Rect)> {
		self.do_get_separator_at_path(area, &path.0)
	}

	fn do_get_separator_at_path(
		&self,
		area: Rect,
		path: &[bool],
	) -> Option<(SplitDirection, Rect)> {
		let Layout::Split {
			direction,
			ratio,
			first,
			second,
		} = self
		else {
			return None;
		};

		let (first_area, second_area, sep_rect) =
			Self::compute_split_areas(area, *direction, *ratio);

		if path.is_empty() {
			return Some((*direction, sep_rect));
		}

		if path[0] {
			second.do_get_separator_at_path(second_area, &path[1..])
		} else {
			first.do_get_separator_at_path(first_area, &path[1..])
		}
	}

	pub(super) fn compute_split_areas(
		area: Rect,
		direction: SplitDirection,
		ratio: f32,
	) -> (Rect, Rect, Rect) {
		match direction {
			SplitDirection::Horizontal => {
				let first_width = ((area.width as f32) * ratio).round() as u16;
				(
					Rect {
						x: area.x,
						y: area.y,
						width: first_width,
						height: area.height,
					},
					Rect {
						x: area.x + first_width + 1,
						y: area.y,
						width: area.width.saturating_sub(first_width).saturating_sub(1),
						height: area.height,
					},
					Rect {
						x: area.x + first_width,
						y: area.y,
						width: 1,
						height: area.height,
					},
				)
			}
			SplitDirection::Vertical => {
				let first_height = ((area.height as f32) * ratio).round() as u16;
				(
					Rect {
						x: area.x,
						y: area.y,
						width: area.width,
						height: first_height,
					},
					Rect {
						x: area.x,
						y: area.y + first_height + 1,
						width: area.width,
						height: area.height.saturating_sub(first_height).saturating_sub(1),
					},
					Rect {
						x: area.x,
						y: area.y + first_height,
						width: area.width,
						height: 1,
					},
				)
			}
		}
	}

	/// Returns separator positions for rendering.
	///
	/// Each tuple contains: (direction, visual_priority, rect).
	/// The visual priority is the maximum of the adjacent views' priorities,
	/// used to determine which background color the separator should use.
	pub fn separator_positions(&self, area: Rect) -> Vec<(SplitDirection, u8, Rect)> {
		let Layout::Split {
			direction,
			ratio,
			first,
			second,
		} = self
		else {
			return vec![];
		};

		let (first_area, second_area, sep_rect) =
			Self::compute_split_areas(area, *direction, *ratio);

		let priority = first
			.last_view()
			.visual_priority()
			.max(second.first_view().visual_priority());

		let mut separators = vec![(*direction, priority, sep_rect)];
		separators.extend(first.separator_positions(first_area));
		separators.extend(second.separator_positions(second_area));
		separators
	}
}
