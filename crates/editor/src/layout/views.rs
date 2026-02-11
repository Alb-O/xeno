//! View management and navigation.
//!
//! Finding, counting, and navigating between views in the layout.

use xeno_tui::layout::Rect;

use super::manager::LayoutManager;
use super::types::LayerId;
use crate::buffer::{Layout, SpatialDirection, ViewId};

impl LayoutManager {
	/// Returns the first view in the layout, searching from the topmost non-empty layer down.
	pub fn first_view(&self, base_layout: &Layout) -> ViewId {
		for i in (1..self.layers.len()).rev() {
			if let Some(layout) = self.layers[i].layout.as_ref() {
				return layout.first_view();
			}
		}
		base_layout.first_view()
	}

	/// Returns the first text buffer identifier if one exists.
	pub fn first_buffer(&self, base_layout: &Layout) -> Option<ViewId> {
		for i in (1..self.layers.len()).rev() {
			if let Some(layout) = self.layers[i].layout.as_ref()
				&& let Some(id) = layout.first_buffer()
			{
				return Some(id);
			}
		}
		base_layout.first_buffer()
	}

	/// Returns the total number of views across all layers.
	pub fn count(&self, base_layout: &Layout) -> usize {
		let overlay_count: usize = self.layers.iter().skip(1).filter_map(|slot| slot.layout.as_ref()).map(|l| l.count()).sum();
		base_layout.count() + overlay_count
	}

	/// Returns all views present in all layers.
	pub fn views(&self, base_layout: &Layout) -> Vec<ViewId> {
		let mut views = base_layout.views();
		views.extend(self.layers.iter().skip(1).filter_map(|slot| slot.layout.as_ref()).flat_map(|l| l.views()));
		views
	}

	/// Returns all text buffer identifiers across all layers.
	pub fn buffer_ids(&self, base_layout: &Layout) -> Vec<ViewId> {
		let mut ids = base_layout.buffer_ids();
		ids.extend(self.layers.iter().skip(1).filter_map(|slot| slot.layout.as_ref()).flat_map(|l| l.buffer_ids()));
		ids
	}

	/// Returns `true` if any layer contains the specified view.
	pub fn contains_view(&self, base_layout: &Layout, view: ViewId) -> bool {
		if base_layout.contains_view(view) {
			return true;
		}
		self.layers
			.iter()
			.skip(1)
			.filter_map(|slot| slot.layout.as_ref())
			.any(|l| l.contains_view(view))
	}

	/// Returns the next view in layout order, starting from the current view's layer.
	pub fn next_view(&self, base_layout: &Layout, current: ViewId) -> ViewId {
		if let Some(layer) = self.layer_of_view(base_layout, current)
			&& let Some(layout) = self.overlay_layout(layer)
		{
			return layout.next_view(current);
		}
		base_layout.next_view(current)
	}

	/// Returns the previous view in layout order.
	pub fn prev_view(&self, base_layout: &Layout, current: ViewId) -> ViewId {
		if let Some(layer) = self.layer_of_view(base_layout, current)
			&& let Some(layout) = self.overlay_layout(layer)
		{
			return layout.prev_view(current);
		}
		base_layout.prev_view(current)
	}

	/// Returns the next buffer identifier in layout order.
	pub fn next_buffer(&self, base_layout: &Layout, current: ViewId) -> ViewId {
		base_layout.next_buffer(current)
	}

	/// Returns the previous buffer identifier in layout order.
	pub fn prev_buffer(&self, base_layout: &Layout, current: ViewId) -> ViewId {
		base_layout.prev_buffer(current)
	}

	/// Finds the view at the given screen coordinates, searching layers top-down.
	pub fn view_at_position(&self, base_layout: &Layout, area: Rect, x: u16, y: u16) -> Option<(ViewId, Rect)> {
		for i in (1..self.layers.len()).rev() {
			if let Some(layout) = self.layers[i].layout.as_ref() {
				let layer_id = LayerId::new(i as u16, self.layers[i].generation);
				let layer_area = self.layer_area(layer_id, area);
				if let Some(result) = layout.view_at_position(layer_area, x, y) {
					return Some(result);
				}
			}
		}
		base_layout.view_at_position(area, x, y)
	}

	/// Computes rectangular areas for each view in the base layer.
	pub fn compute_view_areas(&self, base_layout: &Layout, area: Rect) -> Vec<(ViewId, Rect)> {
		base_layout.compute_view_areas(area)
	}

	/// Computes rectangular areas for views in a specific layer.
	pub fn compute_view_areas_for_layer(&self, base_layout: &Layout, layer: LayerId, area: Rect) -> Vec<(ViewId, Rect)> {
		self.layer(base_layout, layer).map(|l| l.compute_view_areas(area)).unwrap_or_default()
	}

	/// Computes rectangular areas for each buffer in the base layer.
	pub fn compute_buffer_areas(&self, base_layout: &Layout, area: Rect) -> Vec<(ViewId, Rect)> {
		base_layout.compute_areas(area)
	}

	/// Finds the view in the given direction, searching within the current view's layer.
	pub fn view_in_direction(&self, base_layout: &Layout, area: Rect, current: ViewId, direction: SpatialDirection, hint: u16) -> Option<ViewId> {
		if let Some(layer) = self.layer_of_view(base_layout, current)
			&& let Some(layout) = self.overlay_layout(layer)
		{
			let layer_area = self.layer_area(layer, area);
			return layout.view_in_direction(layer_area, current, direction, hint);
		}
		base_layout.view_in_direction(area, current, direction, hint)
	}
}
