//! Layout management for buffer splits.
//!
//! The `Layout` enum represents how buffers are arranged in the editor window.
//! It supports recursive splitting for complex layouts.
//!
//! The layout system is view-agnostic: it can contain text buffers, terminals,
//! or any other content type via the `BufferView` enum.

use super::BufferId;

/// Direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
	/// Horizontal split (buffers side by side).
	Horizontal,
	/// Vertical split (buffers stacked).
	Vertical,
}

/// Unique identifier for a terminal buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalId(pub u64);

/// A view in the layout - either a text buffer or a terminal.
///
/// This allows the layout system to manage heterogeneous content types
/// in splits without requiring a common trait object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferView {
	/// A text buffer (document editing).
	Text(BufferId),
	/// A terminal emulator.
	Terminal(TerminalId),
}

impl BufferView {
	/// Returns the text buffer ID if this is a text view.
	pub fn as_text(&self) -> Option<BufferId> {
		match self {
			BufferView::Text(id) => Some(*id),
			BufferView::Terminal(_) => None,
		}
	}

	/// Returns the terminal ID if this is a terminal view.
	pub fn as_terminal(&self) -> Option<TerminalId> {
		match self {
			BufferView::Text(_) => None,
			BufferView::Terminal(id) => Some(*id),
		}
	}

	/// Returns true if this is a text buffer view.
	pub fn is_text(&self) -> bool {
		matches!(self, BufferView::Text(_))
	}

	/// Returns true if this is a terminal view.
	pub fn is_terminal(&self) -> bool {
		matches!(self, BufferView::Terminal(_))
	}
}

impl From<BufferId> for BufferView {
	fn from(id: BufferId) -> Self {
		BufferView::Text(id)
	}
}

impl From<TerminalId> for BufferView {
	fn from(id: TerminalId) -> Self {
		BufferView::Terminal(id)
	}
}

/// Layout tree for buffer arrangement.
///
/// Each node is either a single view (text buffer or terminal) or a split
/// containing two child layouts.
#[derive(Debug, Clone)]
pub enum Layout {
	/// A single buffer view (text or terminal).
	Single(BufferView),
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

	/// Returns the first view in the layout.
	///
	/// For splits, this returns the first view found (leftmost/topmost).
	pub fn first_view(&self) -> BufferView {
		match self {
			Layout::Single(view) => *view,
			Layout::Split { first, .. } => first.first_view(),
		}
	}

	/// Returns the first text buffer ID if one exists.
	///
	/// For splits, traverses leftmost/topmost first.
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

	/// Replaces a view with a new layout (for splitting).
	///
	/// Returns true if the replacement was made.
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

	/// Replaces a buffer ID with a new layout (for splitting).
	///
	/// Returns true if the replacement was made.
	pub fn replace(&mut self, target: BufferId, new_layout: Layout) -> bool {
		self.replace_view(BufferView::Text(target), new_layout)
	}

	/// Removes a view from the layout, collapsing splits as needed.
	///
	/// Returns the new layout if the view was found and removed,
	/// or None if removing would leave no views.
	pub fn remove_view(&self, target: BufferView) -> Option<Layout> {
		match self {
			Layout::Single(view) if *view == target => None,
			Layout::Single(_) => Some(self.clone()),
			Layout::Split {
				direction,
				ratio,
				first,
				second,
			} => {
				let first_removed = first.remove_view(target);
				let second_removed = second.remove_view(target);

				match (first_removed, second_removed) {
					(None, None) => None,
					(Some(layout), None) | (None, Some(layout)) => Some(layout),
					(Some(f), Some(s)) => Some(Layout::Split {
						direction: *direction,
						ratio: *ratio,
						first: Box::new(f),
						second: Box::new(s),
					}),
				}
			}
		}
	}

	/// Removes a buffer from the layout, collapsing splits as needed.
	///
	/// Returns the new layout if the buffer was found and removed,
	/// or None if removing would leave no buffers.
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

	/// Returns the next view in the layout order.
	///
	/// Used for `Ctrl+w w` navigation.
	pub fn next_view(&self, current: BufferView) -> BufferView {
		let views = self.views();
		if views.is_empty() {
			return current;
		}

		let current_idx = views.iter().position(|&v| v == current).unwrap_or(0);
		let next_idx = (current_idx + 1) % views.len();
		views[next_idx]
	}

	/// Returns the previous view in the layout order.
	pub fn prev_view(&self, current: BufferView) -> BufferView {
		let views = self.views();
		if views.is_empty() {
			return current;
		}

		let current_idx = views.iter().position(|&v| v == current).unwrap_or(0);
		let prev_idx = if current_idx == 0 {
			views.len() - 1
		} else {
			current_idx - 1
		};
		views[prev_idx]
	}

	/// Returns the next buffer ID in the layout order (text buffers only).
	///
	/// Used for `:bnext` navigation.
	pub fn next_buffer(&self, current: BufferId) -> BufferId {
		let ids = self.buffer_ids();
		if ids.is_empty() {
			return current;
		}

		let current_idx = ids.iter().position(|&id| id == current).unwrap_or(0);
		let next_idx = (current_idx + 1) % ids.len();
		ids[next_idx]
	}

	/// Returns the previous buffer ID in the layout order (text buffers only).
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

	/// Computes the rectangular areas for each view in the layout.
	///
	/// Returns a vec of (BufferView, Rect) pairs representing the screen area
	/// assigned to each view.
	pub fn compute_view_areas(
		&self,
		area: ratatui::layout::Rect,
	) -> Vec<(BufferView, ratatui::layout::Rect)> {
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

	/// Computes the rectangular areas for each buffer in the layout.
	///
	/// Returns a vec of (BufferId, Rect) pairs representing the screen area
	/// assigned to each buffer.
	pub fn compute_areas(
		&self,
		area: ratatui::layout::Rect,
	) -> Vec<(BufferId, ratatui::layout::Rect)> {
		self.compute_view_areas(area)
			.into_iter()
			.filter_map(|(view, rect)| view.as_text().map(|id| (id, rect)))
			.collect()
	}

	/// Helper to split an area according to direction and ratio.
	fn split_area(
		area: ratatui::layout::Rect,
		direction: SplitDirection,
		ratio: f32,
	) -> (ratatui::layout::Rect, ratatui::layout::Rect) {
		match direction {
			SplitDirection::Horizontal => {
				let first_width = ((area.width as f32) * ratio).round() as u16;
				let second_width = area.width.saturating_sub(first_width).saturating_sub(1);
				let first_rect = ratatui::layout::Rect {
					x: area.x,
					y: area.y,
					width: first_width,
					height: area.height,
				};
				let second_rect = ratatui::layout::Rect {
					x: area.x + first_width + 1,
					y: area.y,
					width: second_width,
					height: area.height,
				};
				(first_rect, second_rect)
			}
			SplitDirection::Vertical => {
				let first_height = ((area.height as f32) * ratio).round() as u16;
				let second_height = area.height.saturating_sub(first_height).saturating_sub(1);
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
					height: second_height,
				};
				(first_rect, second_rect)
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
		assert_eq!(layout.first_buffer(), Some(BufferId(1)));
		assert_eq!(layout.buffer_ids(), vec![BufferId(1)]);
		assert!(layout.contains(BufferId(1)));
		assert!(!layout.contains(BufferId(2)));
	}

	#[test]
	fn test_hsplit() {
		let layout = Layout::hsplit(Layout::single(BufferId(1)), Layout::single(BufferId(2)));

		assert_eq!(layout.first_buffer(), Some(BufferId(1)));
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
