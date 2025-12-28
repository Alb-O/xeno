//! Layout management for buffer splits.
//!
//! The [`LayoutManager`] owns the layout tree and handles all split operations,
//! view navigation, and separator interactions (hover/drag for resizing).
//!
//! # Responsibilities
//!
//! - Store and modify the layout tree
//! - Handle split creation and view removal
//! - Compute view areas for rendering
//! - Track separator hover/drag state for resize operations
//! - Navigate between views (next/prev)
//!
//! # Not Responsible For
//!
//! - Buffer/terminal storage (handled by [`BufferManager`])
//! - Focus tracking (handled by [`BufferManager`])
//! - Rendering separators (handled by render code, using state from here)
//!
//! [`BufferManager`]: super::BufferManager

use evildoer_tui::layout::Rect;

use super::separator::{DragState, MouseVelocityTracker, SeparatorHoverAnimation};
use crate::buffer::{BufferId, BufferView, Layout, SplitDirection, SplitPath, TerminalId};

/// Manages the layout tree and separator interactions.
///
/// This struct owns the layout tree that determines how views are arranged
/// in splits, and tracks state for separator hover/drag interactions.
pub struct LayoutManager {
	/// The layout tree defining how views are arranged.
	layout: Layout,

	/// Currently hovered separator (for visual feedback during resize).
	///
	/// Contains the separator's direction and screen rectangle when the mouse
	/// is hovering over a split boundary. Only set when velocity is low enough.
	pub hovered_separator: Option<(SplitDirection, Rect)>,

	/// Separator the mouse is currently over (regardless of velocity).
	///
	/// This tracks the physical position even when hover is suppressed due to
	/// fast mouse movement, allowing us to activate hover when mouse slows down.
	pub separator_under_mouse: Option<(SplitDirection, Rect)>,

	/// Animation state for separator hover fade effects.
	///
	/// Tracks ongoing hover animations for smooth visual transitions.
	pub separator_hover_animation: Option<SeparatorHoverAnimation>,

	/// Tracks mouse velocity to suppress hover effects during fast movement.
	pub mouse_velocity: MouseVelocityTracker,

	/// Active separator drag state for resizing splits.
	///
	/// When dragging a separator, this contains the separator's direction
	/// and the path to identify which split is being resized.
	pub dragging_separator: Option<DragState>,

	/// Tracks the view where a text selection drag started.
	///
	/// When the user starts a mouse drag for text selection, this records
	/// which view the drag originated in. Drag events are only processed
	/// for this view, preventing selection from crossing split boundaries.
	/// Cleared on mouse up.
	pub text_selection_origin: Option<(BufferView, Rect)>,
}

impl LayoutManager {
	/// Creates a new layout manager with a single text buffer.
	pub fn new(buffer_id: BufferId) -> Self {
		Self {
			layout: Layout::text(buffer_id),
			hovered_separator: None,
			separator_under_mouse: None,
			separator_hover_animation: None,
			mouse_velocity: MouseVelocityTracker::default(),
			dragging_separator: None,
			text_selection_origin: None,
		}
	}

	/// Returns a reference to the layout tree.
	pub fn layout(&self) -> &Layout {
		&self.layout
	}

	/// Returns a mutable reference to the layout tree.
	///
	/// Use sparingly - prefer the higher-level methods when possible.
	pub fn layout_mut(&mut self) -> &mut Layout {
		&mut self.layout
	}

	/// Returns the first view in the layout (leftmost/topmost).
	pub fn first_view(&self) -> BufferView {
		self.layout.first_view()
	}

	/// Returns the first text buffer ID if one exists.
	pub fn first_buffer(&self) -> Option<BufferId> {
		self.layout.first_buffer()
	}

	/// Returns the number of views in the layout.
	pub fn count(&self) -> usize {
		self.layout.count()
	}

	/// Returns all views in the layout.
	pub fn views(&self) -> Vec<BufferView> {
		self.layout.views()
	}

	/// Returns all text buffer IDs in the layout.
	pub fn buffer_ids(&self) -> Vec<BufferId> {
		self.layout.buffer_ids()
	}

	/// Returns all terminal IDs in the layout.
	pub fn terminal_ids(&self) -> Vec<TerminalId> {
		self.layout.terminal_ids()
	}

	/// Checks if the layout contains a specific view.
	pub fn contains_view(&self, view: BufferView) -> bool {
		self.layout.contains_view(view)
	}

	/// Returns the next view in the layout order (for `Ctrl+w w` navigation).
	pub fn next_view(&self, current: BufferView) -> BufferView {
		self.layout.next_view(current)
	}

	/// Returns the previous view in the layout order.
	pub fn prev_view(&self, current: BufferView) -> BufferView {
		self.layout.prev_view(current)
	}

	/// Returns the next buffer ID in layout order (for `:bnext`).
	pub fn next_buffer(&self, current: BufferId) -> BufferId {
		self.layout.next_buffer(current)
	}

	/// Returns the previous buffer ID in layout order (for `:bprev`).
	pub fn prev_buffer(&self, current: BufferId) -> BufferId {
		self.layout.prev_buffer(current)
	}

	/// Creates a horizontal split with a new buffer below the current view.
	///
	/// The split line is horizontal. Current view stays on top, new buffer below.
	/// Matches Vim's `:split` and Helix's `hsplit`.
	pub fn split_horizontal(&mut self, current_view: BufferView, new_buffer_id: BufferId) {
		let new_layout = Layout::stacked(Layout::single(current_view), Layout::text(new_buffer_id));
		self.layout.replace_view(current_view, new_layout);
	}

	/// Creates a vertical split with a new buffer to the right of the current view.
	///
	/// The split line is vertical. Current view stays on left, new buffer on right.
	/// Matches Vim's `:vsplit` and Helix's `vsplit`.
	pub fn split_vertical(&mut self, current_view: BufferView, new_buffer_id: BufferId) {
		let new_layout =
			Layout::side_by_side(Layout::single(current_view), Layout::text(new_buffer_id));
		self.layout.replace_view(current_view, new_layout);
	}

	/// Creates a horizontal split with a new terminal below the current view.
	///
	/// The split line is horizontal. Current view stays on top, terminal below.
	pub fn split_horizontal_terminal(&mut self, current_view: BufferView, terminal_id: TerminalId) {
		let new_layout =
			Layout::stacked(Layout::single(current_view), Layout::terminal(terminal_id));
		self.layout.replace_view(current_view, new_layout);
	}

	/// Creates a vertical split with a new terminal to the right of the current view.
	///
	/// The split line is vertical. Current view stays on left, terminal on right.
	pub fn split_vertical_terminal(&mut self, current_view: BufferView, terminal_id: TerminalId) {
		let new_layout =
			Layout::side_by_side(Layout::single(current_view), Layout::terminal(terminal_id));
		self.layout.replace_view(current_view, new_layout);
	}

	/// Removes a view from the layout, collapsing splits as needed.
	///
	/// Returns the new focused view if the layout was modified, or None if
	/// the view wasn't found or removing would leave no views.
	pub fn remove_view(&mut self, view: BufferView) -> Option<BufferView> {
		// Don't remove the last view
		if self.layout.count() <= 1 {
			return None;
		}

		if let Some(new_layout) = self.layout.remove_view(view) {
			self.layout = new_layout;
			// Return the first view as a fallback focus target
			Some(self.layout.first_view())
		} else {
			None
		}
	}

	/// Computes rectangular areas for each view in the layout.
	pub fn compute_view_areas(&self, area: Rect) -> Vec<(BufferView, Rect)> {
		self.layout.compute_view_areas(area)
	}

	/// Computes rectangular areas for each buffer in the layout.
	pub fn compute_buffer_areas(&self, area: Rect) -> Vec<(BufferId, Rect)> {
		self.layout.compute_areas(area)
	}

	/// Finds the view at the given screen coordinates.
	pub fn view_at_position(&self, area: Rect, x: u16, y: u16) -> Option<(BufferView, Rect)> {
		self.layout.view_at_position(area, x, y)
	}

	/// Returns separator positions for rendering.
	pub fn separator_positions(&self, area: Rect) -> Vec<(SplitDirection, u16, Rect)> {
		self.layout.separator_positions(area)
	}

	/// Finds the separator at the given screen coordinates.
	pub fn separator_at_position(
		&self,
		area: Rect,
		x: u16,
		y: u16,
	) -> Option<(SplitDirection, Rect)> {
		self.layout.separator_at_position(area, x, y)
	}

	/// Finds the separator and its path at the given screen coordinates.
	pub fn separator_with_path_at_position(
		&self,
		area: Rect,
		x: u16,
		y: u16,
	) -> Option<(SplitDirection, Rect, SplitPath)> {
		self.layout.separator_with_path_at_position(area, x, y)
	}

	/// Gets the separator rect for a split at the given path.
	pub fn separator_rect_at_path(
		&self,
		area: Rect,
		path: &SplitPath,
	) -> Option<(SplitDirection, Rect)> {
		self.layout.separator_rect_at_path(area, path)
	}

	/// Resizes the split at the given path based on mouse position.
	pub fn resize_at_path(&mut self, area: Rect, path: &SplitPath, mouse_x: u16, mouse_y: u16) {
		self.layout.resize_at_path(area, path, mouse_x, mouse_y);
	}

	/// Updates the mouse velocity tracker with a new position.
	pub fn update_mouse_velocity(&mut self, x: u16, y: u16) {
		self.mouse_velocity.update(x, y);
	}

	/// Returns true if the mouse is moving fast enough to suppress hover effects.
	pub fn is_mouse_fast(&self) -> bool {
		self.mouse_velocity.is_fast()
	}

	/// Starts a separator drag operation.
	pub fn start_drag(&mut self, direction: SplitDirection, path: SplitPath, separator_rect: Rect) {
		self.dragging_separator = Some(DragState { direction, path });
		let old_hover = self.hovered_separator.take();
		self.hovered_separator = Some((direction, separator_rect));
		if old_hover != self.hovered_separator {
			self.update_hover_animation(old_hover, self.hovered_separator);
		}
	}

	/// Ends the current separator drag operation.
	pub fn end_drag(&mut self) {
		self.dragging_separator = None;
		self.hovered_separator = None;
	}

	/// Returns true if a separator drag is active.
	pub fn is_dragging(&self) -> bool {
		self.dragging_separator.is_some()
	}

	/// Returns the current drag state, if any.
	pub fn drag_state(&self) -> Option<&DragState> {
		self.dragging_separator.as_ref()
	}

	/// Updates the separator hover animation when hover state changes.
	pub fn update_hover_animation(
		&mut self,
		old: Option<(SplitDirection, Rect)>,
		new: Option<(SplitDirection, Rect)>,
	) {
		use crate::test_events::{AnimationDirection, SeparatorAnimationEvent};

		match (old, new) {
			(None, Some((_, rect))) => {
				// Started hovering - animate in
				SeparatorAnimationEvent::start(AnimationDirection::FadeIn);
				self.separator_hover_animation = Some(SeparatorHoverAnimation::new(rect, true));
			}
			(Some((_, old_rect)), None) => {
				// Stopped hovering - animate out from current position
				let can_toggle = self
					.separator_hover_animation
					.as_ref()
					.map(|a| a.rect == old_rect)
					.unwrap_or(false);
				if can_toggle {
					// Same separator - just toggle the existing animation
					SeparatorAnimationEvent::start(AnimationDirection::FadeOut);
					self.separator_hover_animation
						.as_mut()
						.unwrap()
						.set_hovering(false);
					return;
				}
				// Different separator or no existing animation - create new one at full intensity
				SeparatorAnimationEvent::start(AnimationDirection::FadeOut);
				self.separator_hover_animation = Some(SeparatorHoverAnimation::new_at_intensity(
					old_rect, 1.0, false,
				));
			}
			(Some((_, old_rect)), Some((_, new_rect))) if old_rect != new_rect => {
				// Moved to a different separator - start fresh animation
				SeparatorAnimationEvent::start(AnimationDirection::FadeIn);
				self.separator_hover_animation = Some(SeparatorHoverAnimation::new(new_rect, true));
			}
			_ => {
				// Same separator or both None - no change needed
			}
		}
	}

	/// Returns true if the hover animation needs a redraw.
	pub fn animation_needs_redraw(&self) -> bool {
		self.separator_hover_animation
			.as_ref()
			.map(|a| a.needs_redraw())
			.unwrap_or(false)
	}

	/// Returns the animation intensity for the given separator rect.
	pub fn animation_intensity(&self) -> f32 {
		self.separator_hover_animation
			.as_ref()
			.map(|a| a.intensity())
			.unwrap_or(0.0)
	}

	/// Returns the rect being animated, if any.
	pub fn animation_rect(&self) -> Option<Rect> {
		self.separator_hover_animation.as_ref().map(|a| a.rect)
	}
}
