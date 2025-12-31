//! Layer management for stacked layouts.
//!
//! Layers are ordered from bottom (index 0) to top. Layer 0 is the base layer
//! which always exists. Higher layers overlay on top with transparent backgrounds.

use evildoer_tui::layout::Rect;

use super::manager::LayoutManager;
use super::types::LayerIndex;
use crate::buffer::{BufferView, Layout};

impl LayoutManager {
	/// Returns the base layer (index 0), which always exists.
	pub(super) fn base_layer(&self) -> &Layout {
		self.layers[0].as_ref().expect("base layer always exists")
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

	/// Finds which layer contains a view.
	pub fn layer_of_view(&self, view: BufferView) -> Option<LayerIndex> {
		self.layers
			.iter()
			.enumerate()
			.find(|(_, l)| l.as_ref().is_some_and(|l| l.contains_view(view)))
			.map(|(i, _)| i)
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
	pub(super) fn effective_dock_height(&self, total_height: u16) -> u16 {
		let max_dock = total_height.saturating_sub(Self::MIN_DOCK_HEIGHT);
		self.dock_height.clamp(Self::MIN_DOCK_HEIGHT, max_dock)
	}

	/// Returns the effective side dock width, clamped to available space.
	pub(super) fn effective_side_dock_width(&self, total_width: u16) -> u16 {
		let max_dock = total_width.saturating_sub(Self::MIN_SIDE_DOCK_WIDTH);
		self.side_dock_width
			.clamp(Self::MIN_SIDE_DOCK_WIDTH, max_dock)
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
}
