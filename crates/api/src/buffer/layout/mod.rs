//! Layout management for buffer splits.
//!
//! The `Layout` enum represents how buffers are arranged in the editor window.
//! It supports recursive splitting for complex layouts.
//!
//! The layout system is view-agnostic: it can contain text buffers, terminals,
//! or any other content type via the `BufferView` enum.
//!
//! Split positions are stored as absolute screen coordinates, not ratios.
//! This ensures splits remain stable when other UI elements (like the dock
//! terminal) appear or disappear.

mod areas;
mod navigation;
#[cfg(test)]
mod tests;
mod types;

pub use types::{BufferView, SplitDirection, SplitPath, TerminalId};

use evildoer_tui::layout::Rect;

use super::BufferId;

/// Layout tree for buffer arrangement.
///
/// Represents how views (text buffers and terminals) are arranged in splits.
/// The layout is a binary tree where leaves are single views and internal
/// nodes are splits.
///
/// # Structure
///
/// ```text
/// Layout::Split
/// ├── first: Layout::Single(BufferView::Text(1))
/// └── second: Layout::Split
///     ├── first: Layout::Single(BufferView::Text(2))
///     └── second: Layout::Single(BufferView::Terminal(1))
/// ```
///
/// # Creating Layouts
///
/// Splits require the current view area to compute absolute separator positions:
///
/// ```ignore
/// let layout = Layout::side_by_side(
///     Layout::text(buffer_id),
///     Layout::terminal(terminal_id),
///     view_area,
/// );
/// ```
#[derive(Debug, Clone)]
pub enum Layout {
	/// A single buffer view (text or terminal).
	Single(BufferView),
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
	/// Creates a new single-view layout from any view type.
	pub fn single(view: impl Into<BufferView>) -> Self {
		Layout::Single(view.into())
	}

	/// Creates a new single-view layout for a text buffer.
	pub fn text(buffer_id: BufferId) -> Self {
		Layout::Single(BufferView::Text(buffer_id))
	}

	/// Creates a new single-view layout for a terminal.
	pub fn terminal(terminal_id: TerminalId) -> Self {
		Layout::Single(BufferView::Terminal(terminal_id))
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

	/// Returns the first view in the layout (leftmost/topmost).
	pub fn first_view(&self) -> BufferView {
		match self {
			Layout::Single(view) => *view,
			Layout::Split { first, .. } => first.first_view(),
		}
	}

	/// Returns the last view in the layout (rightmost/bottommost).
	pub fn last_view(&self) -> BufferView {
		match self {
			Layout::Single(view) => *view,
			Layout::Split { second, .. } => second.last_view(),
		}
	}

	/// Returns the first text buffer ID if one exists.
	pub fn first_buffer(&self) -> Option<BufferId> {
		match self {
			Layout::Single(BufferView::Text(id)) => Some(*id),
			Layout::Single(BufferView::Terminal(_)) => None,
			Layout::Split { first, second, .. } => {
				first.first_buffer().or_else(|| second.first_buffer())
			}
		}
	}

	/// Returns all views in this layout.
	pub fn views(&self) -> Vec<BufferView> {
		match self {
			Layout::Single(view) => vec![*view],
			Layout::Split { first, second, .. } => {
				let mut views = first.views();
				views.extend(second.views());
				views
			}
		}
	}

	/// Returns all text buffer IDs in this layout.
	pub fn buffer_ids(&self) -> Vec<BufferId> {
		self.views()
			.into_iter()
			.filter_map(|v| v.as_text())
			.collect()
	}

	/// Returns all terminal IDs in this layout.
	pub fn terminal_ids(&self) -> Vec<TerminalId> {
		self.views()
			.into_iter()
			.filter_map(|v| v.as_terminal())
			.collect()
	}

	/// Checks if this layout contains a specific view.
	pub fn contains_view(&self, view: BufferView) -> bool {
		match self {
			Layout::Single(v) => *v == view,
			Layout::Split { first, second, .. } => {
				first.contains_view(view) || second.contains_view(view)
			}
		}
	}

	/// Checks if this layout contains a specific text buffer.
	pub fn contains(&self, buffer_id: BufferId) -> bool {
		self.contains_view(BufferView::Text(buffer_id))
	}

	/// Checks if this layout contains a specific terminal.
	pub fn contains_terminal(&self, terminal_id: TerminalId) -> bool {
		self.contains_view(BufferView::Terminal(terminal_id))
	}

	/// Replaces a view with a new layout (for splitting). Returns true if replaced.
	pub fn replace_view(&mut self, target: BufferView, new_layout: Layout) -> bool {
		match self {
			Layout::Single(view) if *view == target => {
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

	/// Replaces a buffer ID with a new layout (for splitting). Returns true if replaced.
	pub fn replace(&mut self, target: BufferId, new_layout: Layout) -> bool {
		self.replace_view(BufferView::Text(target), new_layout)
	}

	/// Removes a view from the layout, collapsing splits as needed.
	/// Returns None if removing would leave no views.
	pub fn remove_view(&self, target: BufferView) -> Option<Layout> {
		match self {
			Layout::Single(view) if *view == target => None,
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

	/// Removes a buffer from the layout, collapsing splits as needed.
	pub fn remove(&self, target: BufferId) -> Option<Layout> {
		self.remove_view(BufferView::Text(target))
	}

	/// Removes a terminal from the layout, collapsing splits as needed.
	pub fn remove_terminal(&self, target: TerminalId) -> Option<Layout> {
		self.remove_view(BufferView::Terminal(target))
	}

	/// Counts the number of views in this layout.
	pub fn count(&self) -> usize {
		match self {
			Layout::Single(_) => 1,
			Layout::Split { first, second, .. } => first.count() + second.count(),
		}
	}
}
