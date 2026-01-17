//! Area computation and separator handling for layout splits.

use xeno_tui::layout::Rect;

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
				position,
				first,
				second,
			} => {
				let (first_area, second_area, _) =
					Self::compute_split_areas(area, *direction, *position);
				let mut areas = first.compute_view_areas(first_area);
				areas.extend(second.compute_view_areas(second_area));
				areas
			}
		}
	}

	/// Computes rectangular areas for each buffer in the layout.
	pub fn compute_areas(&self, area: Rect) -> Vec<(BufferId, Rect)> {
		self.compute_view_areas(area)
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

	/// Recursively searches for a separator at coordinates, building the path.
	fn find_separator_with_path(
		&self,
		area: Rect,
		x: u16,
		y: u16,
		current_path: SplitPath,
	) -> Option<(SplitDirection, Rect, SplitPath)> {
		let Layout::Split {
			direction,
			position,
			first,
			second,
		} = self
		else {
			return None;
		};

		let (first_area, second_area, sep_rect) =
			Self::compute_split_areas(area, *direction, *position);

		if x >= sep_rect.x
			&& x < sep_rect.x + sep_rect.width
			&& y >= sep_rect.y
			&& y < sep_rect.y + sep_rect.height
		{
			return Some((*direction, sep_rect, current_path));
		}

		let mut first_path = current_path.clone();
		first_path.0.push(false);
		if let Some(result) = first.find_separator_with_path(first_area, x, y, first_path) {
			return Some(result);
		}

		let mut second_path = current_path;
		second_path.0.push(true);
		second.find_separator_with_path(second_area, x, y, second_path)
	}

	/// Computes the minimum width this layout requires.
	///
	/// For a single view, this is `MIN_WIDTH`.
	/// For a horizontal split, it's the sum of both children's min widths plus 1 for separator.
	/// For a vertical split, it's the max of both children's min widths.
	pub fn min_width(&self) -> u16 {
		match self {
			Layout::Single(_) => Self::MIN_WIDTH,
			Layout::Split {
				direction,
				first,
				second,
				..
			} => match direction {
				SplitDirection::Horizontal => first.min_width() + 1 + second.min_width(),
				SplitDirection::Vertical => first.min_width().max(second.min_width()),
			},
		}
	}

	/// Computes the minimum height this layout requires.
	///
	/// For a single view, this is `MIN_HEIGHT`.
	/// For a vertical split, it's the sum of both children's min heights plus 1 for separator.
	/// For a horizontal split, it's the max of both children's min heights.
	pub fn min_height(&self) -> u16 {
		match self {
			Layout::Single(_) => Self::MIN_HEIGHT,
			Layout::Split {
				direction,
				first,
				second,
				..
			} => match direction {
				SplitDirection::Vertical => first.min_height() + 1 + second.min_height(),
				SplitDirection::Horizontal => first.min_height().max(second.min_height()),
			},
		}
	}

	/// Resizes the split at the given path based on mouse position.
	pub fn resize_at_path(
		&mut self,
		area: Rect,
		path: &SplitPath,
		mouse_x: u16,
		mouse_y: u16,
	) -> bool {
		self.do_resize_at_path(area, &path.0, mouse_x, mouse_y)
	}

	/// Converts global mouse coordinates to parent-local position and updates the split.
	fn do_resize_at_path(&mut self, area: Rect, path: &[bool], mouse_x: u16, mouse_y: u16) -> bool {
		let Layout::Split {
			direction,
			position,
			first,
			second,
		} = self
		else {
			return false;
		};

		if path.is_empty() {
			*position = match direction {
				SplitDirection::Horizontal => {
					let mouse_local = mouse_x.saturating_sub(area.x);
					let min_pos = first.min_width();
					let max_pos = area.width.saturating_sub(second.min_width() + 1);
					mouse_local.clamp(min_pos.min(max_pos), max_pos)
				}
				SplitDirection::Vertical => {
					let mouse_local = mouse_y.saturating_sub(area.y);
					let min_pos = first.min_height();
					let max_pos = area.height.saturating_sub(second.min_height() + 1);
					mouse_local.clamp(min_pos.min(max_pos), max_pos)
				}
			};
			return true;
		}

		let (first_area, second_area, _) = Self::compute_split_areas(area, *direction, *position);
		if path[0] {
			second.do_resize_at_path(second_area, &path[1..], mouse_x, mouse_y)
		} else {
			first.do_resize_at_path(first_area, &path[1..], mouse_x, mouse_y)
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

	/// Internal recursive implementation of path-based separator lookup.
	fn do_get_separator_at_path(
		&self,
		area: Rect,
		path: &[bool],
	) -> Option<(SplitDirection, Rect)> {
		let Layout::Split {
			direction,
			position,
			first,
			second,
		} = self
		else {
			return None;
		};

		let (first_area, second_area, sep_rect) =
			Self::compute_split_areas(area, *direction, *position);

		if path.is_empty() {
			return Some((*direction, sep_rect));
		}

		if path[0] {
			second.do_get_separator_at_path(second_area, &path[1..])
		} else {
			first.do_get_separator_at_path(first_area, &path[1..])
		}
	}

	/// Computes the areas for a split given parent-local separator position.
	///
	/// Returns (first_area, second_area, separator_rect).
	/// The separator position is clamped to ensure both areas meet minimum size requirements.
	/// The `position` parameter is an offset from the parent origin (0..width-1 or 0..height-1).
	pub(super) fn compute_split_areas(
		area: Rect,
		direction: SplitDirection,
		position_local: u16,
	) -> (Rect, Rect, Rect) {
		match direction {
			SplitDirection::Horizontal => {
				let min_pos = Self::MIN_WIDTH;
				let max_pos = area.width.saturating_sub(Self::MIN_WIDTH + 1);
				let pos = position_local.clamp(min_pos.min(max_pos), max_pos);
				let sep_x = area.x + pos;
				let first_width = pos;
				(
					Rect {
						x: area.x,
						y: area.y,
						width: first_width,
						height: area.height,
					},
					Rect {
						x: sep_x + 1,
						y: area.y,
						width: area.width.saturating_sub(first_width).saturating_sub(1),
						height: area.height,
					},
					Rect {
						x: sep_x,
						y: area.y,
						width: 1,
						height: area.height,
					},
				)
			}
			SplitDirection::Vertical => {
				let min_pos = Self::MIN_HEIGHT;
				let max_pos = area.height.saturating_sub(Self::MIN_HEIGHT + 1);
				let pos = position_local.clamp(min_pos.min(max_pos), max_pos);
				let sep_y = area.y + pos;
				let first_height = pos;
				(
					Rect {
						x: area.x,
						y: area.y,
						width: area.width,
						height: first_height,
					},
					Rect {
						x: area.x,
						y: sep_y + 1,
						width: area.width,
						height: area.height.saturating_sub(first_height).saturating_sub(1),
					},
					Rect {
						x: area.x,
						y: sep_y,
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
	/// Visual priority is always 0 for text-only layouts (no overlay panels).
	pub fn separator_positions(&self, area: Rect) -> Vec<(SplitDirection, u8, Rect)> {
		let Layout::Split {
			direction,
			position,
			first,
			second,
		} = self
		else {
			return vec![];
		};

		let (first_area, second_area, sep_rect) =
			Self::compute_split_areas(area, *direction, *position);

		let mut separators = vec![(*direction, 0, sep_rect)];
		separators.extend(first.separator_positions(first_area));
		separators.extend(second.separator_positions(second_area));
		separators
	}
}
