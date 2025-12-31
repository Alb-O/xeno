//! View management and navigation.
//!
//! Finding, counting, and navigating between views in the layout.

use evildoer_tui::layout::Rect;

use super::manager::LayoutManager;
use crate::buffer::{BufferId, BufferView};

impl LayoutManager {
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

	/// Computes rectangular areas for each view in the base layer.
	pub fn compute_view_areas(&self, area: Rect) -> Vec<(BufferView, Rect)> {
		self.base_layer().compute_view_areas(area)
	}

	/// Computes rectangular areas for views in a specific layer.
	pub fn compute_view_areas_for_layer(
		&self,
		layer: super::types::LayerIndex,
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
}
