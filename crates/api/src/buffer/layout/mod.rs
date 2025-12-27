//! Layout management for buffer splits.
//!
//! The `Layout` enum represents how buffers are arranged in the editor window.
//! It supports recursive splitting for complex layouts.
//!
//! The layout system is view-agnostic: it can contain text buffers, terminals,
//! or any other content type via the `BufferView` enum.

#[cfg(test)]
mod tests;

use super::BufferId;

/// Path to a split in the layout tree.
///
/// Each element indicates which branch to take: `false` for first child,
/// `true` for second child. An empty path refers to the root split.
///
/// This provides a stable way to identify splits that doesn't change
/// when ratios are adjusted during resize operations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SplitPath(pub Vec<bool>);

/// Direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
	/// Horizontal split (buffers side by side).
	Horizontal,
	/// Vertical split (buffers stacked).
	Vertical,
}

/// Unique identifier for a terminal buffer.
///
/// Terminal IDs are assigned sequentially starting from 1 when terminals
/// are created via [`Editor::split_horizontal_terminal`] or
/// [`Editor::split_vertical_terminal`].
///
/// [`Editor::split_horizontal_terminal`]: crate::Editor::split_horizontal_terminal
/// [`Editor::split_vertical_terminal`]: crate::Editor::split_vertical_terminal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalId(pub u64);

/// A view in the layout - either a text buffer or a terminal.
///
/// This enum enables the layout system to manage heterogeneous content types
/// in splits. The editor tracks the focused view via this type, allowing
/// seamless navigation between text editing and terminal sessions.
///
/// # Focus Handling
///
/// When a terminal is focused, text-editing operations are unavailable.
/// Use [`Editor::is_text_focused`] or [`Editor::is_terminal_focused`] to
/// check focus type before operations.
///
/// [`Editor::is_text_focused`]: crate::Editor::is_text_focused
/// [`Editor::is_terminal_focused`]: crate::Editor::is_terminal_focused
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferView {
	/// A text buffer for document editing.
	Text(BufferId),
	/// An embedded terminal emulator.
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
/// Use the constructor methods rather than building variants directly:
///
/// ```ignore
/// let layout = Layout::hsplit(
///     Layout::text(buffer_id),
///     Layout::terminal(terminal_id),
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
		/// Ratio of space given to first child (0.0 to 1.0).
		ratio: f32,
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

	/// Returns the first view in the layout (leftmost/topmost).
	pub fn first_view(&self) -> BufferView {
		match self {
			Layout::Single(view) => *view,
			Layout::Split { first, .. } => first.first_view(),
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
				ratio,
				first,
				second,
			} => match (first.remove_view(target), second.remove_view(target)) {
				(None, None) => None,
				(Some(layout), None) | (None, Some(layout)) => Some(layout),
				(Some(f), Some(s)) => Some(Layout::Split {
					direction: *direction,
					ratio: *ratio,
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

	/// Returns the next view in the layout order (for `Ctrl+w w` navigation).
	pub fn next_view(&self, current: BufferView) -> BufferView {
		let views = self.views();
		if views.is_empty() {
			return current;
		}
		let idx = views.iter().position(|&v| v == current).unwrap_or(0);
		views[(idx + 1) % views.len()]
	}

	/// Returns the previous view in the layout order.
	pub fn prev_view(&self, current: BufferView) -> BufferView {
		let views = self.views();
		if views.is_empty() {
			return current;
		}
		let idx = views.iter().position(|&v| v == current).unwrap_or(0);
		views[if idx == 0 { views.len() - 1 } else { idx - 1 }]
	}

	/// Returns the next buffer ID in layout order (for `:bnext`).
	pub fn next_buffer(&self, current: BufferId) -> BufferId {
		let ids = self.buffer_ids();
		if ids.is_empty() {
			return current;
		}
		let idx = ids.iter().position(|&id| id == current).unwrap_or(0);
		ids[(idx + 1) % ids.len()]
	}

	/// Returns the previous buffer ID in layout order (for `:bprev`).
	pub fn prev_buffer(&self, current: BufferId) -> BufferId {
		let ids = self.buffer_ids();
		if ids.is_empty() {
			return current;
		}
		let idx = ids.iter().position(|&id| id == current).unwrap_or(0);
		ids[if idx == 0 { ids.len() - 1 } else { idx - 1 }]
	}

	/// Finds the view at the given screen coordinates.
	pub fn view_at_position(
		&self,
		area: tome_tui::layout::Rect,
		x: u16,
		y: u16,
	) -> Option<(BufferView, tome_tui::layout::Rect)> {
		self.compute_view_areas(area).into_iter().find(|(_, rect)| {
			x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
		})
	}

	/// Computes rectangular areas for each view in the layout.
	pub fn compute_view_areas(
		&self,
		area: tome_tui::layout::Rect,
	) -> Vec<(BufferView, tome_tui::layout::Rect)> {
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
	pub fn compute_areas(
		&self,
		area: tome_tui::layout::Rect,
	) -> Vec<(BufferId, tome_tui::layout::Rect)> {
		self.compute_view_areas(area)
			.into_iter()
			.filter_map(|(view, rect)| view.as_text().map(|id| (id, rect)))
			.collect()
	}

	/// Helper to split an area according to direction and ratio.
	fn split_area(
		area: tome_tui::layout::Rect,
		direction: SplitDirection,
		ratio: f32,
	) -> (tome_tui::layout::Rect, tome_tui::layout::Rect) {
		let (first, second, _) = Self::compute_split_areas(area, direction, ratio);
		(first, second)
	}

	/// Finds the separator at the given screen coordinates.
	pub fn separator_at_position(
		&self,
		area: tome_tui::layout::Rect,
		x: u16,
		y: u16,
	) -> Option<(SplitDirection, tome_tui::layout::Rect)> {
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
		area: tome_tui::layout::Rect,
		x: u16,
		y: u16,
	) -> Option<(SplitDirection, tome_tui::layout::Rect, SplitPath)> {
		self.find_separator_with_path(area, x, y, SplitPath::default())
	}

	fn find_separator_with_path(
		&self,
		area: tome_tui::layout::Rect,
		x: u16,
		y: u16,
		current_path: SplitPath,
	) -> Option<(SplitDirection, tome_tui::layout::Rect, SplitPath)> {
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
		area: tome_tui::layout::Rect,
		path: &SplitPath,
		mouse_x: u16,
		mouse_y: u16,
	) -> bool {
		self.do_resize_at_path(area, &path.0, mouse_x, mouse_y)
	}

	fn do_resize_at_path(
		&mut self,
		area: tome_tui::layout::Rect,
		path: &[bool],
		mouse_x: u16,
		mouse_y: u16,
	) -> bool {
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

	fn collect_separator_positions(
		&self,
		area: tome_tui::layout::Rect,
	) -> Vec<(SplitDirection, u16)> {
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

	fn adjust_ratios_for_new_area(
		&mut self,
		old_area: tome_tui::layout::Rect,
		new_area: tome_tui::layout::Rect,
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

	fn separator_count(&self) -> usize {
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
		area: tome_tui::layout::Rect,
		path: &SplitPath,
	) -> Option<(SplitDirection, tome_tui::layout::Rect)> {
		self.do_get_separator_at_path(area, &path.0)
	}

	fn do_get_separator_at_path(
		&self,
		area: tome_tui::layout::Rect,
		path: &[bool],
	) -> Option<(SplitDirection, tome_tui::layout::Rect)> {
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

	fn compute_split_areas(
		area: tome_tui::layout::Rect,
		direction: SplitDirection,
		ratio: f32,
	) -> (
		tome_tui::layout::Rect,
		tome_tui::layout::Rect,
		tome_tui::layout::Rect,
	) {
		match direction {
			SplitDirection::Horizontal => {
				let first_width = ((area.width as f32) * ratio).round() as u16;
				(
					tome_tui::layout::Rect {
						x: area.x,
						y: area.y,
						width: first_width,
						height: area.height,
					},
					tome_tui::layout::Rect {
						x: area.x + first_width + 1,
						y: area.y,
						width: area.width.saturating_sub(first_width).saturating_sub(1),
						height: area.height,
					},
					tome_tui::layout::Rect {
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
					tome_tui::layout::Rect {
						x: area.x,
						y: area.y,
						width: area.width,
						height: first_height,
					},
					tome_tui::layout::Rect {
						x: area.x,
						y: area.y + first_height + 1,
						width: area.width,
						height: area.height.saturating_sub(first_height).saturating_sub(1),
					},
					tome_tui::layout::Rect {
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
	pub fn separator_positions(
		&self,
		area: tome_tui::layout::Rect,
	) -> Vec<(SplitDirection, u16, tome_tui::layout::Rect)> {
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

		let mut separators = vec![(*direction, sep_rect.x, sep_rect)];
		separators.extend(first.separator_positions(first_area));
		separators.extend(second.separator_positions(second_area));
		separators
	}
}
