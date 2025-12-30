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
use crate::buffer::{BufferId, BufferView, Layout, SplitDirection, SplitPath};

/// Layer index for layout operations.
pub type LayerIndex = usize;

/// Identifies which separator is being interacted with.
#[derive(Debug, Clone, PartialEq)]
pub enum SeparatorId {
	/// A separator within a layer's split tree.
	Split { path: SplitPath, layer: LayerIndex },
	/// The boundary between layer 0 and layer 1 (bottom dock boundary).
	LayerBoundary,
	/// The boundary between layer 0 and layer 2 (side dock boundary).
	SideBoundary,
}

/// Information about a separator found at a screen position.
#[derive(Debug, Clone)]
pub struct SeparatorHit {
	pub id: SeparatorId,
	pub direction: SplitDirection,
	pub rect: Rect,
}

/// Manages stacked layout layers and separator interactions.
///
/// Layouts are organized in ordered layers. Layer 0 is the base (opaque),
/// higher layers overlay on top with transparent backgrounds.
pub struct LayoutManager {
	/// Layout layers, index 0 is base (bottom), higher indices overlay on top.
	layers: Vec<Option<Layout>>,

	/// Dock layer height in terminal rows (layer 1, bottom). Persists across window resizes.
	dock_height: u16,

	/// Side dock width in terminal columns (layer 2, right). Persists across window resizes.
	side_dock_width: u16,

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
	/// Default dock height in rows.
	const DEFAULT_DOCK_HEIGHT: u16 = 12;

	/// Minimum dock height in rows.
	const MIN_DOCK_HEIGHT: u16 = 3;

	/// Default side dock width in columns.
	const DEFAULT_SIDE_DOCK_WIDTH: u16 = 60;

	/// Minimum side dock width in columns.
	const MIN_SIDE_DOCK_WIDTH: u16 = 20;

	/// Creates a new layout manager with a single text buffer on the base layer.
	pub fn new(buffer_id: BufferId) -> Self {
		Self {
			layers: vec![Some(Layout::text(buffer_id))],
			dock_height: Self::DEFAULT_DOCK_HEIGHT,
			side_dock_width: Self::DEFAULT_SIDE_DOCK_WIDTH,
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
			if let Some(layout) = &self.layers[i]
				&& let Some(id) = layout.first_buffer()
			{
				return Some(id);
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
		if let Some(layer_idx) = self.layer_of_view(current)
			&& let Some(layout) = &self.layers[layer_idx]
		{
			return layout.next_view(current);
		}
		self.base_layer().next_view(current)
	}

	/// Returns the previous view in layout order.
	pub fn prev_view(&self, current: BufferView) -> BufferView {
		if let Some(layer_idx) = self.layer_of_view(current)
			&& let Some(layout) = &self.layers[layer_idx]
		{
			return layout.prev_view(current);
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
	pub fn split_horizontal(
		&mut self,
		current_view: BufferView,
		new_buffer_id: BufferId,
		doc_area: Rect,
	) {
		let Some(view_area) = self.view_area(current_view, doc_area) else {
			return;
		};
		let new_layout = Layout::stacked(
			Layout::single(current_view),
			Layout::text(new_buffer_id),
			view_area,
		);
		if let Some(layer_idx) = self.layer_of_view(current_view)
			&& let Some(layout) = self.layer_mut(layer_idx)
		{
			layout.replace_view(current_view, new_layout);
		}
	}

	/// Creates a vertical split with a new buffer to the right of the current view.
	pub fn split_vertical(
		&mut self,
		current_view: BufferView,
		new_buffer_id: BufferId,
		doc_area: Rect,
	) {
		let Some(view_area) = self.view_area(current_view, doc_area) else {
			return;
		};
		let new_layout = Layout::side_by_side(
			Layout::single(current_view),
			Layout::text(new_buffer_id),
			view_area,
		);
		if let Some(layer_idx) = self.layer_of_view(current_view)
			&& let Some(layout) = self.layer_mut(layer_idx)
		{
			layout.replace_view(current_view, new_layout);
		}
	}

	/// Gets the area of a specific view.
	fn view_area(&self, view: BufferView, doc_area: Rect) -> Option<Rect> {
		let layer_idx = self.layer_of_view(view)?;
		let layer_area = self.layer_area(layer_idx, doc_area);
		self.layers[layer_idx]
			.as_ref()?
			.compute_view_areas(layer_area)
			.into_iter()
			.find(|(v, _)| *v == view)
			.map(|(_, area)| area)
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
	/// Layer 0 gets the full area (shrunk if dock layers are visible).
	/// Layer 1 (bottom dock) gets the bottom portion based on `dock_height`.
	/// Layer 2 (side dock) gets the right portion based on `side_dock_width`.
	pub fn layer_area(&self, layer: LayerIndex, doc_area: Rect) -> Rect {
		if layer == 0 {
			let mut area = doc_area;
			if self.layer(1).is_some() {
				let dock_height = self.effective_dock_height(doc_area.height);
				area.height = area.height.saturating_sub(dock_height);
			}
			if self.layer(2).is_some() {
				let dock_width = self.effective_side_dock_width(doc_area.width);
				area.width = area.width.saturating_sub(dock_width);
			}
			area
		} else if layer == 1 {
			let dock_height = self.effective_dock_height(doc_area.height);
			let mut width = doc_area.width;
			if self.layer(2).is_some() {
				width = width.saturating_sub(self.effective_side_dock_width(doc_area.width));
			}
			Rect {
				x: doc_area.x,
				y: doc_area.bottom().saturating_sub(dock_height),
				width,
				height: dock_height,
			}
		} else if layer == 2 {
			let dock_width = self.effective_side_dock_width(doc_area.width);
			Rect {
				x: doc_area.right().saturating_sub(dock_width),
				y: doc_area.y,
				width: dock_width,
				height: doc_area.height,
			}
		} else {
			doc_area
		}
	}

	/// Returns the effective dock height, clamped to available space.
	fn effective_dock_height(&self, total_height: u16) -> u16 {
		let max_dock = total_height.saturating_sub(Self::MIN_DOCK_HEIGHT);
		self.dock_height.clamp(Self::MIN_DOCK_HEIGHT, max_dock)
	}

	/// Returns the effective side dock width, clamped to available space.
	fn effective_side_dock_width(&self, total_width: u16) -> u16 {
		let max_dock = total_width.saturating_sub(Self::MIN_SIDE_DOCK_WIDTH);
		self.side_dock_width.clamp(Self::MIN_SIDE_DOCK_WIDTH, max_dock)
	}

	/// Returns the separator rect between layer 0 and layer 1 (the bottom dock boundary).
	///
	/// Returns None if layer 1 is not visible.
	pub fn layer_boundary_separator(&self, doc_area: Rect) -> Option<Rect> {
		self.layer(1)?;
		let layer0_area = self.layer_area(0, doc_area);
		// Adjust width if side dock is visible
		let width = if self.layer(2).is_some() {
			doc_area.width.saturating_sub(self.effective_side_dock_width(doc_area.width))
		} else {
			doc_area.width
		};
		Some(Rect {
			x: doc_area.x,
			y: layer0_area.bottom(),
			width,
			height: 1,
		})
	}

	/// Returns the separator rect between layer 0/1 and layer 2 (the side dock boundary).
	///
	/// Returns None if layer 2 is not visible.
	pub fn side_boundary_separator(&self, doc_area: Rect) -> Option<Rect> {
		self.layer(2)?;
		let layer2_area = self.layer_area(2, doc_area);
		Some(Rect {
			x: layer2_area.x.saturating_sub(1),
			y: doc_area.y,
			width: 1,
			height: doc_area.height,
		})
	}

	/// Resizes the dock layer by moving the boundary to the given y position.
	pub fn resize_dock_boundary(&mut self, doc_area: Rect, mouse_y: u16) {
		let new_dock_top = mouse_y.saturating_sub(doc_area.y);
		let new_height = doc_area.height.saturating_sub(new_dock_top);
		let max_dock = doc_area.height.saturating_sub(Self::MIN_DOCK_HEIGHT);
		self.dock_height = new_height.clamp(Self::MIN_DOCK_HEIGHT, max_dock);
	}

	/// Resizes the side dock layer by moving the boundary to the given x position.
	pub fn resize_side_boundary(&mut self, doc_area: Rect, mouse_x: u16) {
		let new_width = doc_area.right().saturating_sub(mouse_x);
		let max_dock = doc_area.width.saturating_sub(Self::MIN_SIDE_DOCK_WIDTH);
		self.side_dock_width = new_width.clamp(Self::MIN_SIDE_DOCK_WIDTH, max_dock);
	}

	/// Returns the visual priority for the layer boundary separator.
	///
	/// This is the max priority of the bottom view of layer 0 and top view of layer 1.
	pub fn layer_boundary_priority(&self) -> u8 {
		let layer0_priority = self
			.layer(0)
			.map(|l| l.last_view().visual_priority())
			.unwrap_or(0);
		let layer1_priority = self
			.layer(1)
			.map(|l| l.first_view().visual_priority())
			.unwrap_or(0);
		layer0_priority.max(layer1_priority)
	}

	/// Returns the visual priority for the side boundary separator.
	pub fn side_boundary_priority(&self) -> u8 {
		self.layer(2)
			.map(|l| l.first_view().visual_priority())
			.unwrap_or(0)
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
	pub fn separator_positions(&self, area: Rect) -> Vec<(SplitDirection, u8, Rect)> {
		self.base_layer().separator_positions(area)
	}

	/// Returns separator positions for a specific layer.
	pub fn separator_positions_for_layer(
		&self,
		layer: LayerIndex,
		area: Rect,
	) -> Vec<(SplitDirection, u8, Rect)> {
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
	/// Checks layer boundaries first, then searches split separators top-down.
	pub fn separator_hit_at_position(&self, area: Rect, x: u16, y: u16) -> Option<SeparatorHit> {
		// Check side boundary (layer 2) first - it's a vertical line
		if let Some(rect) = self.side_boundary_separator(area)
			&& x == rect.x
			&& y >= rect.y
			&& y < rect.bottom()
		{
			return Some(SeparatorHit {
				id: SeparatorId::SideBoundary,
				direction: SplitDirection::Horizontal,
				rect,
			});
		}

		// Check bottom dock boundary (layer 1) - it's a horizontal line
		if let Some(rect) = self.layer_boundary_separator(area)
			&& y == rect.y
			&& x >= rect.x
			&& x < rect.right()
		{
			return Some(SeparatorHit {
				id: SeparatorId::LayerBoundary,
				direction: SplitDirection::Vertical,
				rect,
			});
		}

		for i in (0..self.layers.len()).rev() {
			if let Some(layout) = &self.layers[i] {
				let layer_area = self.layer_area(i, area);
				if let Some((direction, rect, path)) =
					layout.separator_with_path_at_position(layer_area, x, y)
				{
					return Some(SeparatorHit {
						id: SeparatorId::Split { path, layer: i },
						direction,
						rect,
					});
				}
			}
		}
		None
	}

	/// Gets the separator rect for the given separator ID.
	pub fn separator_rect(&self, area: Rect, id: &SeparatorId) -> Option<Rect> {
		match id {
			SeparatorId::Split { path, layer } => {
				let layer_area = self.layer_area(*layer, area);
				self.layer(*layer)?
					.separator_rect_at_path(layer_area, path)
					.map(|(_, rect)| rect)
			}
			SeparatorId::LayerBoundary => self.layer_boundary_separator(area),
			SeparatorId::SideBoundary => self.side_boundary_separator(area),
		}
	}

	/// Resizes the separator identified by the given ID based on mouse position.
	pub fn resize_separator(&mut self, area: Rect, id: &SeparatorId, mouse_x: u16, mouse_y: u16) {
		match id {
			SeparatorId::Split { path, layer } => {
				let layer_area = self.layer_area(*layer, area);
				if let Some(layout) = self.layer_mut(*layer) {
					layout.resize_at_path(layer_area, path, mouse_x, mouse_y);
				}
			}
			SeparatorId::LayerBoundary => {
				self.resize_dock_boundary(area, mouse_y);
			}
			SeparatorId::SideBoundary => {
				self.resize_side_boundary(area, mouse_x);
			}
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

	/// Starts a separator drag operation.
	pub fn start_drag(&mut self, hit: &SeparatorHit) {
		self.dragging_separator = Some(DragState { id: hit.id.clone() });
		let old_hover = self.hovered_separator.take();
		self.hovered_separator = Some((hit.direction, hit.rect));
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

#[cfg(test)]
mod tests {
	use evildoer_manifest::PanelId;

	use super::*;

	fn make_doc_area() -> Rect {
		Rect {
			x: 0,
			y: 0,
			width: 80,
			height: 24,
		}
	}

	fn test_panel_id() -> PanelId {
		PanelId::new(0, 0)
	}

	#[test]
	fn layer_area_base_only() {
		let mgr = LayoutManager::new(BufferId(0));
		let doc = make_doc_area();

		let layer0 = mgr.layer_area(0, doc);
		assert_eq!(layer0, doc, "base layer gets full area when no dock");
	}

	#[test]
	fn layer_area_with_dock() {
		let mut mgr = LayoutManager::new(BufferId(0));
		mgr.set_layer(1, Some(Layout::panel(test_panel_id())));
		let doc = make_doc_area();

		let layer0 = mgr.layer_area(0, doc);
		let layer1 = mgr.layer_area(1, doc);

		assert!(
			layer0.height < doc.height,
			"base layer shrinks when dock visible"
		);
		assert!(layer1.height > 0, "dock layer has height");
		assert_eq!(
			layer0.height + layer1.height,
			doc.height,
			"layers fill doc area"
		);
		assert_eq!(
			layer1.y,
			layer0.bottom(),
			"dock starts at base layer bottom"
		);
	}

	#[test]
	fn layer_boundary_separator() {
		let mut mgr = LayoutManager::new(BufferId(0));
		let doc = make_doc_area();

		assert!(
			mgr.layer_boundary_separator(doc).is_none(),
			"no boundary without dock"
		);

		mgr.set_layer(1, Some(Layout::panel(test_panel_id())));
		let boundary = mgr.layer_boundary_separator(doc).unwrap();

		assert_eq!(boundary.x, doc.x);
		assert_eq!(boundary.width, doc.width);
		assert_eq!(boundary.height, 1);
		assert_eq!(boundary.y, mgr.layer_area(0, doc).bottom());
	}

	#[test]
	fn separator_hit_layer_boundary() {
		let mut mgr = LayoutManager::new(BufferId(0));
		mgr.set_layer(1, Some(Layout::panel(test_panel_id())));
		let doc = make_doc_area();

		let boundary = mgr.layer_boundary_separator(doc).unwrap();
		let hit = mgr.separator_hit_at_position(doc, boundary.x + 5, boundary.y);

		assert!(hit.is_some());
		let hit = hit.unwrap();
		assert!(matches!(hit.id, SeparatorId::LayerBoundary));
		assert_eq!(hit.rect, boundary);
	}

	#[test]
	fn resize_dock_boundary() {
		let mut mgr = LayoutManager::new(BufferId(0));
		mgr.set_layer(1, Some(Layout::panel(test_panel_id())));
		let doc = make_doc_area();

		let initial_height = mgr.layer_area(1, doc).height;

		// Drag boundary up (increases dock height)
		mgr.resize_dock_boundary(doc, doc.y + 8);
		let new_height = mgr.layer_area(1, doc).height;
		assert!(
			new_height > initial_height,
			"dragging up increases dock height"
		);

		// Drag boundary down (decreases dock height)
		mgr.resize_dock_boundary(doc, doc.y + 20);
		let newer_height = mgr.layer_area(1, doc).height;
		assert!(
			newer_height < new_height,
			"dragging down decreases dock height"
		);
	}

	#[test]
	fn view_at_position_searches_top_down() {
		let mut mgr = LayoutManager::new(BufferId(0));
		mgr.set_layer(1, Some(Layout::panel(test_panel_id())));
		let doc = make_doc_area();

		let layer1_area = mgr.layer_area(1, doc);
		let mid_x = layer1_area.x + layer1_area.width / 2;
		let mid_y = layer1_area.y + layer1_area.height / 2;

		let hit = mgr.view_at_position(doc, mid_x, mid_y);
		assert!(hit.is_some());
		let (view, _) = hit.unwrap();
		assert!(
			matches!(view, BufferView::Panel(_)),
			"clicking in dock area returns panel"
		);

		let layer0_area = mgr.layer_area(0, doc);
		let hit = mgr.view_at_position(doc, layer0_area.x + 5, layer0_area.y + 5);
		assert!(hit.is_some());
		let (view, _) = hit.unwrap();
		assert!(
			matches!(view, BufferView::Text(_)),
			"clicking in base area returns buffer"
		);
	}

	#[test]
	fn dock_height_clamps() {
		let mut mgr = LayoutManager::new(BufferId(0));
		mgr.set_layer(1, Some(Layout::panel(test_panel_id())));
		let doc = make_doc_area();

		// Try to make dock too small
		mgr.resize_dock_boundary(doc, doc.bottom() - 1);
		let height = mgr.layer_area(1, doc).height;
		assert!(
			height >= LayoutManager::MIN_DOCK_HEIGHT,
			"dock height respects minimum"
		);

		// Try to make dock too large
		mgr.resize_dock_boundary(doc, doc.y + 1);
		let height = mgr.layer_area(1, doc).height;
		let max = doc.height - LayoutManager::MIN_DOCK_HEIGHT;
		assert!(height <= max, "dock height respects maximum");
	}
}
