//! View management and navigation.
//!
//! Finding, counting, and navigating between views in the layout.

use xeno_tui::layout::Rect;

use super::manager::LayoutManager;
use crate::buffer::{BufferId, BufferView, Layout, SpatialDirection};

impl LayoutManager {
	/// Returns the first view in the layout (from topmost non-empty layer).
	pub fn first_view(&self, base_layout: &Layout) -> BufferView {
		for i in (1..self.layers.len()).rev() {
			if let Some(layout) = &self.layers[i] {
				return layout.first_view();
			}
		}
		base_layout.first_view()
	}

	/// Returns the first text buffer ID if one exists (searches all layers).
	pub fn first_buffer(&self, base_layout: &Layout) -> Option<BufferId> {
		for i in (1..self.layers.len()).rev() {
			if let Some(layout) = &self.layers[i]
				&& let Some(id) = layout.first_buffer()
			{
				return Some(id);
			}
		}
		base_layout.first_buffer()
	}

	/// Returns the number of views across all layers.
	pub fn count(&self, base_layout: &Layout) -> usize {
		let overlay_count: usize = self
			.layers
			.iter()
			.skip(1)
			.filter_map(|l| l.as_ref())
			.map(|l| l.count())
			.sum();
		base_layout.count() + overlay_count
	}

	/// Returns all views across all layers.
	pub fn views(&self, base_layout: &Layout) -> Vec<BufferView> {
		let mut views = base_layout.views();
		views.extend(
			self.layers
				.iter()
				.skip(1)
				.filter_map(|l| l.as_ref())
				.flat_map(|l| l.views()),
		);
		views
	}

	/// Returns all text buffer IDs across all layers.
	pub fn buffer_ids(&self, base_layout: &Layout) -> Vec<BufferId> {
		let mut ids = base_layout.buffer_ids();
		ids.extend(
			self.layers
				.iter()
				.skip(1)
				.filter_map(|l| l.as_ref())
				.flat_map(|l| l.buffer_ids()),
		);
		ids
	}

	/// Checks if any layer contains a specific view.
	pub fn contains_view(&self, base_layout: &Layout, view: BufferView) -> bool {
		if base_layout.contains_view(view) {
			return true;
		}
		self.layers
			.iter()
			.skip(1)
			.filter_map(|l| l.as_ref())
			.any(|l| l.contains_view(view))
	}

	/// Returns the next view in layout order (searches current layer first).
	pub fn next_view(&self, base_layout: &Layout, current: BufferView) -> BufferView {
		if let Some(layer_idx) = self.layer_of_view(base_layout, current)
			&& layer_idx != 0
			&& let Some(layout) = &self.layers[layer_idx]
		{
			return layout.next_view(current);
		}
		base_layout.next_view(current)
	}

	/// Returns the previous view in layout order.
	pub fn prev_view(&self, base_layout: &Layout, current: BufferView) -> BufferView {
		if let Some(layer_idx) = self.layer_of_view(base_layout, current)
			&& layer_idx != 0
			&& let Some(layout) = &self.layers[layer_idx]
		{
			return layout.prev_view(current);
		}
		base_layout.prev_view(current)
	}

	/// Returns the next buffer ID in layout order.
	pub fn next_buffer(&self, base_layout: &Layout, current: BufferId) -> BufferId {
		base_layout.next_buffer(current)
	}

	/// Returns the previous buffer ID in layout order.
	pub fn prev_buffer(&self, base_layout: &Layout, current: BufferId) -> BufferId {
		base_layout.prev_buffer(current)
	}

	/// Finds the view at the given screen coordinates (searches top-down).
	pub fn view_at_position(
		&self,
		base_layout: &Layout,
		area: Rect,
		x: u16,
		y: u16,
	) -> Option<(BufferView, Rect)> {
		for i in (1..self.layers.len()).rev() {
			if let Some(layout) = &self.layers[i] {
				let layer_area = self.layer_area(i, area);
				if let Some(result) = layout.view_at_position(layer_area, x, y) {
					return Some(result);
				}
			}
		}
		base_layout.view_at_position(area, x, y)
	}

	/// Computes rectangular areas for each view in the base layer.
	pub fn compute_view_areas(&self, base_layout: &Layout, area: Rect) -> Vec<(BufferView, Rect)> {
		base_layout.compute_view_areas(area)
	}

	/// Computes rectangular areas for views in a specific layer.
	pub fn compute_view_areas_for_layer(
		&self,
		base_layout: &Layout,
		layer: super::types::LayerIndex,
		area: Rect,
	) -> Vec<(BufferView, Rect)> {
		self.layer(base_layout, layer)
			.map(|l| l.compute_view_areas(area))
			.unwrap_or_default()
	}

	/// Computes rectangular areas for each buffer in the base layer.
	pub fn compute_buffer_areas(&self, base_layout: &Layout, area: Rect) -> Vec<(BufferId, Rect)> {
		base_layout.compute_areas(area)
	}

	/// Finds the view in the given direction, searching the current view's layer.
	pub fn view_in_direction(
		&self,
		base_layout: &Layout,
		area: Rect,
		current: BufferView,
		direction: SpatialDirection,
		hint: u16,
	) -> Option<BufferView> {
		if let Some(idx) = self.layer_of_view(base_layout, current)
			&& idx != 0
			&& let Some(layout) = &self.layers[idx]
		{
			return layout.view_in_direction(self.layer_area(idx, area), current, direction, hint);
		}
		base_layout.view_in_direction(area, current, direction, hint)
	}
}
