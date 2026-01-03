//! Layout management for buffer splits.
//!
//! The `Layout` enum represents how buffers are arranged in the editor window.
//! It supports recursive splitting for complex layouts.
//!
//! Split positions are stored as absolute screen coordinates, not ratios.
//! This ensures splits remain stable when other UI elements appear or disappear.

mod areas;
mod navigation;
#[cfg(test)]
mod tests;
mod types;

pub use types::{BufferView, SplitDirection, SplitPath};
use xeno_tui::layout::Rect;

use super::BufferId;

/// Layout tree for buffer arrangement.
///
/// Represents how text buffers are arranged in splits.
/// The layout is a binary tree where leaves are single buffers and internal
/// nodes are splits.
///
/// # Structure
///
/// ```text
/// Layout::Split
/// ├── first: Layout::Single(BufferId(1))
/// └── second: Layout::Split
///     ├── first: Layout::Single(BufferId(2))
///     └── second: Layout::Single(BufferId(3))
/// ```
#[derive(Debug, Clone)]
pub enum Layout {
	/// A single text buffer.
	Single(BufferId),
	/// A split containing two child layouts.
	Split {
		/// Direction of the split (horizontal or vertical).
		direction: SplitDirection,
		/// Absolute position of the separator (x for horizontal, y for vertical).
		position: u16,
		/// First child (left for horizontal, top for vertical).
		first: Box<Layout>,
		/// Second child (right for horizontal, bottom for vertical).
		second: Box<Layout>,
	},
}

impl Layout {
	/// Minimum width for a split view in columns.
	pub const MIN_WIDTH: u16 = 10;

	/// Minimum height for a split view in rows.
	pub const MIN_HEIGHT: u16 = 3;

	/// Creates a new single-buffer layout.
	pub fn single(buffer_id: BufferId) -> Self {
		Layout::Single(buffer_id)
	}

	/// Creates a new single-buffer layout (alias for `single`).
	pub fn text(buffer_id: BufferId) -> Self {
		Layout::Single(buffer_id)
	}

	/// Creates a side-by-side split (first on left, second on right).
	///
	/// The separator is placed at the horizontal center of the given area.
	/// This is a "vertical split" in Vim/Helix terminology (vertical divider line).
	pub fn side_by_side(first: Layout, second: Layout, area: Rect) -> Self {
		Layout::Split {
			direction: SplitDirection::Horizontal,
			position: area.x + area.width / 2,
			first: Box::new(first),
			second: Box::new(second),
		}
	}

	/// Creates a stacked split (first on top, second on bottom).
	///
	/// The separator is placed at the vertical center of the given area.
	/// This is a "horizontal split" in Vim/Helix terminology (horizontal divider line).
	pub fn stacked(first: Layout, second: Layout, area: Rect) -> Self {
		Layout::Split {
			direction: SplitDirection::Vertical,
			position: area.y + area.height / 2,
			first: Box::new(first),
			second: Box::new(second),
		}
	}

	/// Returns the first buffer in the layout (leftmost/topmost).
	pub fn first_view(&self) -> BufferId {
		match self {
			Layout::Single(id) => *id,
			Layout::Split { first, .. } => first.first_view(),
		}
	}

	/// Returns the last buffer in the layout (rightmost/bottommost).
	pub fn last_view(&self) -> BufferId {
		match self {
			Layout::Single(id) => *id,
			Layout::Split { second, .. } => second.last_view(),
		}
	}

	/// Returns the first buffer ID (same as first_view for text-only layouts).
	pub fn first_buffer(&self) -> Option<BufferId> {
		Some(self.first_view())
	}

	/// Returns all buffer IDs in this layout.
	pub fn views(&self) -> Vec<BufferId> {
		match self {
			Layout::Single(id) => vec![*id],
			Layout::Split { first, second, .. } => {
				let mut views = first.views();
				views.extend(second.views());
				views
			}
		}
	}

	/// Returns all buffer IDs in this layout (alias for views).
	pub fn buffer_ids(&self) -> Vec<BufferId> {
		self.views()
	}

	/// Checks if this layout contains a specific buffer.
	pub fn contains_view(&self, buffer_id: BufferId) -> bool {
		match self {
			Layout::Single(id) => *id == buffer_id,
			Layout::Split { first, second, .. } => {
				first.contains_view(buffer_id) || second.contains_view(buffer_id)
			}
		}
	}

	/// Checks if this layout contains a specific buffer (alias for contains_view).
	pub fn contains(&self, buffer_id: BufferId) -> bool {
		self.contains_view(buffer_id)
	}

	/// Replaces a buffer with a new layout (for splitting). Returns true if replaced.
	pub fn replace_view(&mut self, target: BufferId, new_layout: Layout) -> bool {
		match self {
			Layout::Single(id) if *id == target => {
				*self = new_layout;
				true
			}
			Layout::Single(_) => false,
			Layout::Split { first, second, .. } => {
				first.replace_view(target, new_layout.clone())
					|| second.replace_view(target, new_layout)
			}
		}
	}

	/// Replaces a buffer with a new layout (alias for replace_view).
	pub fn replace(&mut self, target: BufferId, new_layout: Layout) -> bool {
		self.replace_view(target, new_layout)
	}

	/// Removes a buffer from the layout, collapsing splits as needed.
	/// Returns None if removing would leave no buffers.
	pub fn remove_view(&self, target: BufferId) -> Option<Layout> {
		match self {
			Layout::Single(id) if *id == target => None,
			Layout::Single(_) => Some(self.clone()),
			Layout::Split {
				direction,
				position,
				first,
				second,
			} => match (first.remove_view(target), second.remove_view(target)) {
				(None, None) => None,
				(Some(layout), None) | (None, Some(layout)) => Some(layout),
				(Some(f), Some(s)) => Some(Layout::Split {
					direction: *direction,
					position: *position,
					first: Box::new(f),
					second: Box::new(s),
				}),
			},
		}
	}

	/// Removes a buffer from the layout (alias for remove_view).
	pub fn remove(&self, target: BufferId) -> Option<Layout> {
		self.remove_view(target)
	}

	/// Counts the number of buffers in this layout.
	pub fn count(&self) -> usize {
		match self {
			Layout::Single(_) => 1,
			Layout::Split { first, second, .. } => first.count() + second.count(),
		}
	}
}
