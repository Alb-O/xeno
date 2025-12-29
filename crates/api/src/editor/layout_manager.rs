//! Layout management for buffer splits.
//!
//! The [`LayoutManager`] owns stacked layout layers and handles all split operations,
//! view navigation, and separator interactions (hover/drag for resizing).
//!
//! # Layer System
//!
//! Layouts are organized in ordered layers (index 0 at bottom):
//! - Layer 0: Base layer (always exists, opaque background)
//! - Layer 1+: Overlay layers (transparent base, rendered on top)
//!
//! Focus goes to the topmost layer containing views by default.
//!
//! # Responsibilities
//!
//! - Store and modify layout layers
//! - Handle split creation and view removal
//! - Compute view areas for rendering (per-layer)
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

/// Layer index for layout operations.
pub type LayerIndex = usize;

/// Information about a separator found at a screen position.
#[derive(Debug, Clone)]
pub enum SeparatorHit {
	/// A separator within a layer's split tree.
	Split {
		direction: SplitDirection,
		rect: Rect,
		path: SplitPath,
		layer: LayerIndex,
	},
	/// The boundary between layer 0 and layer 1 (dock boundary).
	LayerBoundary { rect: Rect },
}

/// Manages stacked layout layers and separator interactions.
///
/// Layouts are organized in ordered layers. Layer 0 is the base (opaque),
/// higher layers overlay on top with transparent backgrounds.
pub struct LayoutManager {
	/// Layout layers, index 0 is base (bottom), higher indices overlay on top.
	layers: Vec<Option<Layout>>,

	/// Dock layer height as a ratio of total height (0.0 to 1.0).
	/// Only used when layer 1 is visible.
	dock_height_ratio: f32,

	/// Currently hovered separator (for visual feedback during resize).
	pub hovered_separator: Option<(SplitDirection, Rect)>,

	/// Separator the mouse is currently over (regardless of velocity).
	pub separator_under_mouse: Option<(SplitDirection, Rect)>,

	/// Animation state for separator hover fade effects.
	pub separator_hover_animation: Option<SeparatorHoverAnimation>,

	/// Tracks mouse velocity to suppress hover effects during fast movement.
	pub mouse_velocity: MouseVelocityTracker,

	/// Active separator drag state for resizing splits.
	pub dragging_separator: Option<DragState>,

	/// Tracks the view where a text selection drag started.
	pub text_selection_origin: Option<(BufferView, Rect)>,
}

impl LayoutManager {
	/// Default dock height as a ratio of total height.
	const DEFAULT_DOCK_HEIGHT_RATIO: f32 = 0.33;

	/// Minimum dock height ratio to prevent dock from disappearing.
	const MIN_DOCK_HEIGHT_RATIO: f32 = 0.1;

	/// Maximum dock height ratio to prevent dock from taking over.
	const MAX_DOCK_HEIGHT_RATIO: f32 = 0.8;

	/// Creates a new layout manager with a single text buffer on the base layer.
	pub fn new(buffer_id: BufferId) -> Self {
		Self {
			layers: vec![Some(Layout::text(buffer_id))],
			dock_height_ratio: Self::DEFAULT_DOCK_HEIGHT_RATIO,
			hovered_separator: None,
			separator_under_mouse: None,
			separator_hover_animation: None,
			mouse_velocity: MouseVelocityTracker::default(),
			dragging_separator: None,
			text_selection_origin: None,
		}
	}

	/// Returns the base layer (index 0), which always exists.
	fn base_layer(&self) -> &Layout {
		self.layers[0].as_ref().expect("base layer always exists")
	}

	/// Returns a mutable reference to the base layer.
	fn base_layer_mut(&mut self) -> &mut Layout {
		self.layers[0].as_mut().expect("base layer always exists")
	}

	/// Returns the layout at a specific layer, if it exists.
	pub fn layer(&self, index: LayerIndex) -> Option<&Layout> {
		self.layers.get(index).and_then(|l| l.as_ref())
	}

	/// Returns a mutable reference to the layout at a specific layer.
	pub fn layer_mut(&mut self, index: LayerIndex) -> Option<&mut Layout> {
		self.layers.get_mut(index).and_then(|l| l.as_mut())
	}

	/// Sets the layout for a layer, creating intermediate layers if needed.
	pub fn set_layer(&mut self, index: LayerIndex, layout: Option<Layout>) {
		while self.layers.len() <= index {
			self.layers.push(None);
		}
		self.layers[index] = layout;
	}

	/// Returns the topmost non-empty layer index.
	pub fn top_layer(&self) -> LayerIndex {
		for i in (0..self.layers.len()).rev() {
			if self.layers[i].is_some() {
				return i;
			}
		}
		0
	}

	/// Returns the number of layers (including empty ones).
	pub fn layer_count(&self) -> usize {
		self.layers.len()
	}

	/// Returns a reference to the layout tree (base layer for compatibility).
	pub fn layout(&self) -> &Layout {
		self.base_layer()
	}

	/// Returns a mutable reference to the layout tree (base layer for compatibility).
	pub fn layout_mut(&mut self) -> &mut Layout {
		self.base_layer_mut()
	}

	/// Returns the first view in the layout (from topmost non-empty layer).
	pub fn first_view(&self) -> BufferView {
		for i in (0..self.layers.len()).rev() {
			if let Some(layout) = &self.layers[i] {
				return layout.first_view();
			}
		}
		self.base_layer().first_view()
	}

	/// Returns the first text buffer ID if one exists (searches all layers).
	pub fn first_buffer(&self) -> Option<BufferId> {
		for i in (0..self.layers.len()).rev() {
			if let Some(layout) = &self.layers[i] {
				if let Some(id) = layout.first_buffer() {
					return Some(id);
				}
			}
		}
		None
	}

	/// Returns the number of views across all layers.
	pub fn count(&self) -> usize {
		self.layers
			.iter()
			.filter_map(|l| l.as_ref())
			.map(|l| l.count())
			.sum()
	}

	/// Returns all views across all layers.
	pub fn views(&self) -> Vec<BufferView> {
		self.layers
			.iter()
			.filter_map(|l| l.as_ref())
			.flat_map(|l| l.views())
			.collect()
	}

	/// Returns all text buffer IDs across all layers.
	pub fn buffer_ids(&self) -> Vec<BufferId> {
		self.layers
			.iter()
			.filter_map(|l| l.as_ref())
			.flat_map(|l| l.buffer_ids())
			.collect()
	}

	/// Returns all terminal IDs across all layers.
	pub fn terminal_ids(&self) -> Vec<TerminalId> {
		self.layers
			.iter()
			.filter_map(|l| l.as_ref())
			.flat_map(|l| l.terminal_ids())
			.collect()
	}

	/// Checks if any layer contains a specific view.
	pub fn contains_view(&self, view: BufferView) -> bool {
		self.layers
			.iter()
			.filter_map(|l| l.as_ref())
			.any(|l| l.contains_view(view))
	}

	/// Finds which layer contains a view.
	pub fn layer_of_view(&self, view: BufferView) -> Option<LayerIndex> {
		self.layers
			.iter()
			.enumerate()
			.find(|(_, l)| l.as_ref().is_some_and(|l| l.contains_view(view)))
			.map(|(i, _)| i)
	}

	/// Returns the next view in layout order (searches current layer first).
	pub fn next_view(&self, current: BufferView) -> BufferView {
		if let Some(layer_idx) = self.layer_of_view(current) {
			if let Some(layout) = &self.layers[layer_idx] {
				return layout.next_view(current);
			}
		}
		self.base_layer().next_view(current)
	}

	/// Returns the previous view in layout order.
	pub fn prev_view(&self, current: BufferView) -> BufferView {
		if let Some(layer_idx) = self.layer_of_view(current) {
			if let Some(layout) = &self.layers[layer_idx] {
				return layout.prev_view(current);
			}
		}
		self.base_layer().prev_view(current)
	}

	/// Returns the next buffer ID in layout order.
	pub fn next_buffer(&self, current: BufferId) -> BufferId {
		self.base_layer().next_buffer(current)
	}

	/// Returns the previous buffer ID in layout order.
	pub fn prev_buffer(&self, current: BufferId) -> BufferId {
		self.base_layer().prev_buffer(current)
	}

	/// Creates a horizontal split with a new buffer below the current view.
	pub fn split_horizontal(&mut self, current_view: BufferView, new_buffer_id: BufferId) {
		let new_layout = Layout::stacked(Layout::single(current_view), Layout::text(new_buffer_id));
		if let Some(layer_idx) = self.layer_of_view(current_view) {
			if let Some(layout) = self.layer_mut(layer_idx) {
				layout.replace_view(current_view, new_layout);
			}
		}
	}

	/// Creates a vertical split with a new buffer to the right of the current view.
	pub fn split_vertical(&mut self, current_view: BufferView, new_buffer_id: BufferId) {
		let new_layout =
			Layout::side_by_side(Layout::single(current_view), Layout::text(new_buffer_id));
		if let Some(layer_idx) = self.layer_of_view(current_view) {
			if let Some(layout) = self.layer_mut(layer_idx) {
				layout.replace_view(current_view, new_layout);
			}
		}
	}

	/// Creates a horizontal split with a new terminal below the current view.
	pub fn split_horizontal_terminal(&mut self, current_view: BufferView, terminal_id: TerminalId) {
		let new_layout =
			Layout::stacked(Layout::single(current_view), Layout::terminal(terminal_id));
		if let Some(layer_idx) = self.layer_of_view(current_view) {
			if let Some(layout) = self.layer_mut(layer_idx) {
				layout.replace_view(current_view, new_layout);
			}
		}
	}

	/// Creates a vertical split with a new terminal to the right of the current view.
	pub fn split_vertical_terminal(&mut self, current_view: BufferView, terminal_id: TerminalId) {
		let new_layout =
			Layout::side_by_side(Layout::single(current_view), Layout::terminal(terminal_id));
		if let Some(layer_idx) = self.layer_of_view(current_view) {
			if let Some(layout) = self.layer_mut(layer_idx) {
				layout.replace_view(current_view, new_layout);
			}
		}
	}

	/// Removes a view from its layer, collapsing splits as needed.
	///
	/// Returns the new focused view if the layout was modified.
	pub fn remove_view(&mut self, view: BufferView) -> Option<BufferView> {
		let layer_idx = self.layer_of_view(view)?;

		// Don't remove the last view from base layer
		if layer_idx == 0 && self.base_layer().count() <= 1 {
			return None;
		}

		let layout = self.layers[layer_idx].as_ref()?;
		let new_layout = layout.remove_view(view);

		if let Some(new_layout) = new_layout {
			self.layers[layer_idx] = Some(new_layout);
			Some(self.layers[layer_idx].as_ref().unwrap().first_view())
		} else {
			// Layer is now empty
			self.layers[layer_idx] = None;
			// Return first view from next non-empty layer
			Some(self.first_view())
		}
	}

	/// Computes rectangular areas for each view in the base layer.
	pub fn compute_view_areas(&self, area: Rect) -> Vec<(BufferView, Rect)> {
		self.base_layer().compute_view_areas(area)
	}

	/// Computes rectangular areas for views in a specific layer.
	pub fn compute_view_areas_for_layer(
		&self,
		layer: LayerIndex,
		area: Rect,
	) -> Vec<(BufferView, Rect)> {
		self.layer(layer)
			.map(|l| l.compute_view_areas(area))
			.unwrap_or_default()
	}

	/// Computes rectangular areas for each buffer in the base layer.
	pub fn compute_buffer_areas(&self, area: Rect) -> Vec<(BufferId, Rect)> {
		self.base_layer().compute_areas(area)
	}

	/// Computes the area for a specific layer given the full doc area.
	///
	/// Layer 0 gets the full area (or shrunk if dock layer is visible).
	/// Layer 1 (dock) gets the bottom portion based on `dock_height_ratio`.
	pub fn layer_area(&self, layer: LayerIndex, doc_area: Rect) -> Rect {
		if layer == 0 {
			if self.layer(1).is_some() {
				let dock_height = (doc_area.height as f32 * self.dock_height_ratio) as u16;
				Rect {
					x: doc_area.x,
					y: doc_area.y,
					width: doc_area.width,
					height: doc_area.height.saturating_sub(dock_height),
				}
			} else {
				doc_area
			}
		} else if layer == 1 {
			let dock_height = (doc_area.height as f32 * self.dock_height_ratio) as u16;
			Rect {
				x: doc_area.x,
				y: doc_area.bottom().saturating_sub(dock_height),
				width: doc_area.width,
				height: dock_height,
			}
		} else {
			doc_area
		}
	}

	/// Returns the separator rect between layer 0 and layer 1 (the dock boundary).
	///
	/// Returns None if layer 1 is not visible.
	pub fn layer_boundary_separator(&self, doc_area: Rect) -> Option<Rect> {
		if self.layer(1).is_none() {
			return None;
		}
		let layer0_area = self.layer_area(0, doc_area);
		Some(Rect {
			x: doc_area.x,
			y: layer0_area.bottom(),
			width: doc_area.width,
			height: 1,
		})
	}

	/// Resizes the dock layer by moving the boundary to the given y position.
	pub fn resize_dock_boundary(&mut self, doc_area: Rect, mouse_y: u16) {
		let total_height = doc_area.height as f32;
		let new_dock_top = mouse_y.saturating_sub(doc_area.y);
		let new_dock_height = doc_area.height.saturating_sub(new_dock_top);
		let new_ratio = new_dock_height as f32 / total_height;
		self.dock_height_ratio =
			new_ratio.clamp(Self::MIN_DOCK_HEIGHT_RATIO, Self::MAX_DOCK_HEIGHT_RATIO);
	}

	/// Finds the view at the given screen coordinates (searches top-down).
	pub fn view_at_position(&self, area: Rect, x: u16, y: u16) -> Option<(BufferView, Rect)> {
		for i in (0..self.layers.len()).rev() {
			if let Some(layout) = &self.layers[i] {
				let layer_area = self.layer_area(i, area);
				if let Some(result) = layout.view_at_position(layer_area, x, y) {
					return Some(result);
				}
			}
		}
		None
	}

	/// Returns separator positions for rendering (base layer).
	pub fn separator_positions(&self, area: Rect) -> Vec<(SplitDirection, u16, Rect)> {
		self.base_layer().separator_positions(area)
	}

	/// Returns separator positions for a specific layer.
	pub fn separator_positions_for_layer(
		&self,
		layer: LayerIndex,
		area: Rect,
	) -> Vec<(SplitDirection, u16, Rect)> {
		self.layer(layer)
			.map(|l| l.separator_positions(area))
			.unwrap_or_default()
	}

	/// Finds the separator at the given screen coordinates (searches top-down).
	pub fn separator_at_position(
		&self,
		area: Rect,
		x: u16,
		y: u16,
	) -> Option<(SplitDirection, Rect)> {
		for i in (0..self.layers.len()).rev() {
			if let Some(layout) = &self.layers[i] {
				let layer_area = self.layer_area(i, area);
				if let Some(result) = layout.separator_at_position(layer_area, x, y) {
					return Some(result);
				}
			}
		}
		None
	}

	/// Finds the separator at the given screen coordinates.
	///
	/// Checks layer boundary first, then searches split separators top-down.
	pub fn separator_hit_at_position(&self, area: Rect, x: u16, y: u16) -> Option<SeparatorHit> {
		// Check layer boundary separator first (between layer 0 and layer 1)
		if let Some(boundary_rect) = self.layer_boundary_separator(area) {
			if y == boundary_rect.y && x >= boundary_rect.x && x < boundary_rect.right() {
				return Some(SeparatorHit::LayerBoundary { rect: boundary_rect });
			}
		}

		// Then check split separators in each layer (top-down)
		for i in (0..self.layers.len()).rev() {
			if let Some(layout) = &self.layers[i] {
				let layer_area = self.layer_area(i, area);
				if let Some((direction, rect, path)) =
					layout.separator_with_path_at_position(layer_area, x, y)
				{
					return Some(SeparatorHit::Split {
						direction,
						rect,
						path,
						layer: i,
					});
				}
			}
		}
		None
	}

	/// Gets the separator rect for a split at the given path in a specific layer.
	pub fn separator_rect_at_path(
		&self,
		area: Rect,
		path: &SplitPath,
		layer: LayerIndex,
	) -> Option<(SplitDirection, Rect)> {
		let layer_area = self.layer_area(layer, area);
		self.layer(layer)?.separator_rect_at_path(layer_area, path)
	}

	/// Resizes the split at the given path in a specific layer based on mouse position.
	pub fn resize_at_path(
		&mut self,
		area: Rect,
		path: &SplitPath,
		layer: LayerIndex,
		mouse_x: u16,
		mouse_y: u16,
	) {
		let layer_area = self.layer_area(layer, area);
		if let Some(layout) = self.layer_mut(layer) {
			layout.resize_at_path(layer_area, path, mouse_x, mouse_y);
		}
	}

	/// Updates the mouse velocity tracker with a new position.
	pub fn update_mouse_velocity(&mut self, x: u16, y: u16) {
		self.mouse_velocity.update(x, y);
	}

	/// Returns true if the mouse is moving fast enough to suppress hover effects.
	pub fn is_mouse_fast(&self) -> bool {
		self.mouse_velocity.is_fast()
	}

	/// Starts a split separator drag operation.
	pub fn start_split_drag(
		&mut self,
		direction: SplitDirection,
		path: SplitPath,
		separator_rect: Rect,
		layer: LayerIndex,
	) {
		self.dragging_separator = Some(DragState::Split {
			direction,
			path,
			layer,
		});
		let old_hover = self.hovered_separator.take();
		self.hovered_separator = Some((direction, separator_rect));
		if old_hover != self.hovered_separator {
			self.update_hover_animation(old_hover, self.hovered_separator);
		}
	}

	/// Starts a layer boundary drag operation (dock resize).
	pub fn start_layer_boundary_drag(&mut self, separator_rect: Rect) {
		self.dragging_separator = Some(DragState::LayerBoundary);
		let old_hover = self.hovered_separator.take();
		self.hovered_separator = Some((SplitDirection::Vertical, separator_rect));
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
