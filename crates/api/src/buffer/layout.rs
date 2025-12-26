//! Layout management for buffer splits.
//!
//! The `Layout` enum represents how buffers are arranged in the editor window.
//! It supports recursive splitting for complex layouts.

use super::BufferId;

/// Direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
	/// Horizontal split (buffers side by side).
	Horizontal,
	/// Vertical split (buffers stacked).
	Vertical,
}

/// Layout tree for buffer arrangement.
///
/// Each node is either a single buffer or a split containing two child layouts.
#[derive(Debug, Clone)]
pub enum Layout {
	/// A single buffer view.
	Single(BufferId),
	/// A split containing two layouts.
	Split {
		/// Direction of the split.
		direction: SplitDirection,
		/// Position of the split (0.0 to 1.0).
		ratio: f32,
		/// First child (left or top).
		first: Box<Layout>,
		/// Second child (right or bottom).
		second: Box<Layout>,
	},
}

impl Layout {
	/// Creates a new single-buffer layout.
	pub fn single(buffer_id: BufferId) -> Self {
		Layout::Single(buffer_id)
	}

	/// Creates a horizontal split (side by side).
	pub fn hsplit(first: Layout, second: Layout) -> Self {
		Layout::Split {
			direction: SplitDirection::Horizontal,
			ratio: 0.5,
			first: Box::new(first),
			second: Box::new(second),
		}
	}

	/// Creates a vertical split (stacked).
	pub fn vsplit(first: Layout, second: Layout) -> Self {
		Layout::Split {
			direction: SplitDirection::Vertical,
			ratio: 0.5,
			first: Box::new(first),
			second: Box::new(second),
		}
	}

	/// Returns the focused buffer ID by traversing the layout.
	///
	/// For splits, this returns the first buffer found (leftmost/topmost).
	/// Use `focused_buffer_at` with a path for proper focus tracking.
	pub fn first_buffer(&self) -> BufferId {
		match self {
			Layout::Single(id) => *id,
			Layout::Split { first, .. } => first.first_buffer(),
		}
	}

	/// Returns all buffer IDs in this layout.
	pub fn buffer_ids(&self) -> Vec<BufferId> {
		match self {
			Layout::Single(id) => vec![*id],
			Layout::Split { first, second, .. } => {
				let mut ids = first.buffer_ids();
				ids.extend(second.buffer_ids());
				ids
			}
		}
	}

	/// Checks if this layout contains a specific buffer.
	pub fn contains(&self, buffer_id: BufferId) -> bool {
		match self {
			Layout::Single(id) => *id == buffer_id,
			Layout::Split { first, second, .. } => {
				first.contains(buffer_id) || second.contains(buffer_id)
			}
		}
	}

	/// Replaces a buffer ID with a new layout (for splitting).
	///
	/// Returns true if the replacement was made.
	pub fn replace(&mut self, target: BufferId, new_layout: Layout) -> bool {
		match self {
			Layout::Single(id) if *id == target => {
				*self = new_layout;
				true
			}
			Layout::Single(_) => false,
			Layout::Split { first, second, .. } => {
				first.replace(target, new_layout.clone()) || second.replace(target, new_layout)
			}
		}
	}

	/// Removes a buffer from the layout, collapsing splits as needed.
	///
	/// Returns the new layout if the buffer was found and removed,
	/// or None if removing would leave no buffers.
	pub fn remove(&self, target: BufferId) -> Option<Layout> {
		match self {
			Layout::Single(id) if *id == target => None,
			Layout::Single(_) => Some(self.clone()),
			Layout::Split { first, second, .. } => {
				let first_removed = first.remove(target);
				let second_removed = second.remove(target);

				match (first_removed, second_removed) {
					(None, None) => None,
					(Some(layout), None) | (None, Some(layout)) => Some(layout),
					(Some(f), Some(s)) => Some(Layout::Split {
						direction: match self {
							Layout::Split { direction, .. } => *direction,
							_ => unreachable!(),
						},
						ratio: match self {
							Layout::Split { ratio, .. } => *ratio,
							_ => unreachable!(),
						},
						first: Box::new(f),
						second: Box::new(s),
					}),
				}
			}
		}
	}

	/// Counts the number of buffer views in this layout.
	pub fn count(&self) -> usize {
		match self {
			Layout::Single(_) => 1,
			Layout::Split { first, second, .. } => first.count() + second.count(),
		}
	}

	/// Returns the next buffer ID in the layout order.
	///
	/// Used for `:bnext` / `Ctrl+w w` navigation.
	pub fn next_buffer(&self, current: BufferId) -> BufferId {
		let ids = self.buffer_ids();
		if ids.is_empty() {
			return current;
		}

		let current_idx = ids.iter().position(|&id| id == current).unwrap_or(0);
		let next_idx = (current_idx + 1) % ids.len();
		ids[next_idx]
	}

	/// Returns the previous buffer ID in the layout order.
	///
	/// Used for `:bprev` navigation.
	pub fn prev_buffer(&self, current: BufferId) -> BufferId {
		let ids = self.buffer_ids();
		if ids.is_empty() {
			return current;
		}

		let current_idx = ids.iter().position(|&id| id == current).unwrap_or(0);
		let prev_idx = if current_idx == 0 {
			ids.len() - 1
		} else {
			current_idx - 1
		};
		ids[prev_idx]
	}

	/// Computes the rectangular areas for each buffer in the layout.
	///
	/// Returns a vec of (BufferId, Rect) pairs representing the screen area
	/// assigned to each buffer.
	pub fn compute_areas(
		&self,
		area: ratatui::layout::Rect,
	) -> Vec<(BufferId, ratatui::layout::Rect)> {
		match self {
			Layout::Single(id) => vec![(*id, area)],
			Layout::Split {
				direction,
				ratio,
				first,
				second,
			} => {
				let (first_area, second_area) = match direction {
					SplitDirection::Horizontal => {
						// Side by side: split width
						let first_width = ((area.width as f32) * ratio).round() as u16;
						let second_width = area.width.saturating_sub(first_width).saturating_sub(1); // -1 for separator
						let first_rect = ratatui::layout::Rect {
							x: area.x,
							y: area.y,
							width: first_width,
							height: area.height,
						};
						let second_rect = ratatui::layout::Rect {
							x: area.x + first_width + 1, // +1 for separator
							y: area.y,
							width: second_width,
							height: area.height,
						};
						(first_rect, second_rect)
					}
					SplitDirection::Vertical => {
						// Stacked: split height
						let first_height = ((area.height as f32) * ratio).round() as u16;
						let second_height =
							area.height.saturating_sub(first_height).saturating_sub(1); // -1 for separator
						let first_rect = ratatui::layout::Rect {
							x: area.x,
							y: area.y,
							width: area.width,
							height: first_height,
						};
						let second_rect = ratatui::layout::Rect {
							x: area.x,
							y: area.y + first_height + 1, // +1 for separator
							width: area.width,
							height: second_height,
						};
						(first_rect, second_rect)
					}
				};

				let mut areas = first.compute_areas(first_area);
				areas.extend(second.compute_areas(second_area));
				areas
			}
		}
	}

	/// Returns the separator positions for rendering.
	///
	/// Each separator is represented as (direction, position) where position
	/// is the x coordinate for horizontal splits or y for vertical splits.
	pub fn separator_positions(
		&self,
		area: ratatui::layout::Rect,
	) -> Vec<(SplitDirection, u16, ratatui::layout::Rect)> {
		match self {
			Layout::Single(_) => vec![],
			Layout::Split {
				direction,
				ratio,
				first,
				second,
			} => {
				let (first_area, second_area, sep_rect) = match direction {
					SplitDirection::Horizontal => {
						let first_width = ((area.width as f32) * ratio).round() as u16;
						let first_rect = ratatui::layout::Rect {
							x: area.x,
							y: area.y,
							width: first_width,
							height: area.height,
						};
						let second_rect = ratatui::layout::Rect {
							x: area.x + first_width + 1,
							y: area.y,
							width: area.width.saturating_sub(first_width).saturating_sub(1),
							height: area.height,
						};
						let sep = ratatui::layout::Rect {
							x: area.x + first_width,
							y: area.y,
							width: 1,
							height: area.height,
						};
						(first_rect, second_rect, sep)
					}
					SplitDirection::Vertical => {
						let first_height = ((area.height as f32) * ratio).round() as u16;
						let first_rect = ratatui::layout::Rect {
							x: area.x,
							y: area.y,
							width: area.width,
							height: first_height,
						};
						let second_rect = ratatui::layout::Rect {
							x: area.x,
							y: area.y + first_height + 1,
							width: area.width,
							height: area.height.saturating_sub(first_height).saturating_sub(1),
						};
						let sep = ratatui::layout::Rect {
							x: area.x,
							y: area.y + first_height,
							width: area.width,
							height: 1,
						};
						(first_rect, second_rect, sep)
					}
				};

				let mut separators = vec![(*direction, sep_rect.x, sep_rect)];
				separators.extend(first.separator_positions(first_area));
				separators.extend(second.separator_positions(second_area));
				separators
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_single_layout() {
		let layout = Layout::single(BufferId(1));
		assert_eq!(layout.first_buffer(), BufferId(1));
		assert_eq!(layout.buffer_ids(), vec![BufferId(1)]);
		assert!(layout.contains(BufferId(1)));
		assert!(!layout.contains(BufferId(2)));
	}

	#[test]
	fn test_hsplit() {
		let layout = Layout::hsplit(Layout::single(BufferId(1)), Layout::single(BufferId(2)));

		assert_eq!(layout.first_buffer(), BufferId(1));
		assert_eq!(layout.buffer_ids(), vec![BufferId(1), BufferId(2)]);
		assert!(layout.contains(BufferId(1)));
		assert!(layout.contains(BufferId(2)));
		assert!(!layout.contains(BufferId(3)));
	}

	#[test]
	fn test_next_prev_buffer() {
		let layout = Layout::hsplit(Layout::single(BufferId(1)), Layout::single(BufferId(2)));

		assert_eq!(layout.next_buffer(BufferId(1)), BufferId(2));
		assert_eq!(layout.next_buffer(BufferId(2)), BufferId(1));
		assert_eq!(layout.prev_buffer(BufferId(1)), BufferId(2));
		assert_eq!(layout.prev_buffer(BufferId(2)), BufferId(1));
	}

	#[test]
	fn test_remove_buffer() {
		let layout = Layout::hsplit(Layout::single(BufferId(1)), Layout::single(BufferId(2)));

		let after_remove = layout.remove(BufferId(1)).unwrap();
		assert_eq!(after_remove.buffer_ids(), vec![BufferId(2)]);

		// Removing the only buffer returns None
		let single = Layout::single(BufferId(1));
		assert!(single.remove(BufferId(1)).is_none());
	}
}
