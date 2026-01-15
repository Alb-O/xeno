//! Layer management for stacked layouts.
//!
//! Layers are ordered from bottom (index 0) to top. Layer 0 is the base layout
//! owned by the base window. Higher layers overlay on top with transparent backgrounds.

use xeno_tui::layout::Rect;

use super::manager::LayoutManager;
use super::types::LayerIndex;
use crate::buffer::{BufferView, Layout};

impl LayoutManager {
	/// Returns the layout at a specific layer, if it exists.
	pub fn layer<'a>(&'a self, base_layout: &'a Layout, index: LayerIndex) -> Option<&'a Layout> {
		if index == 0 {
			return Some(base_layout);
		}
		self.layers.get(index).and_then(|l| l.as_ref())
	}

	/// Returns a mutable reference to the layout at a specific layer.
	pub fn layer_mut<'a>(
		&'a mut self,
		base_layout: &'a mut Layout,
		index: LayerIndex,
	) -> Option<&'a mut Layout> {
		if index == 0 {
			return Some(base_layout);
		}
		self.layers.get_mut(index).and_then(|l| l.as_mut())
	}

	/// Sets the layout for a layer, creating intermediate layers if needed.
	pub fn set_layer(&mut self, index: LayerIndex, layout: Option<Layout>) {
		if index == 0 {
			return;
		}
		while self.layers.len() <= index {
			self.layers.push(None);
		}
		self.layers[index] = layout;
	}

	/// Returns the topmost non-empty layer index.
	pub fn top_layer(&self) -> LayerIndex {
		for i in (1..self.layers.len()).rev() {
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
	pub fn layer_of_view(&self, base_layout: &Layout, view: BufferView) -> Option<LayerIndex> {
		if base_layout.contains_view(view) {
			return Some(0);
		}
		self.layers
			.iter()
			.enumerate()
			.skip(1)
			.find(|(_, l)| l.as_ref().is_some_and(|l| l.contains_view(view)))
			.map(|(i, _)| i)
	}

	/// Computes the area for a specific layer given the full doc area.
	///
	/// Currently all layers get the full doc area (no dock layers).
	pub fn layer_area(&self, _layer: LayerIndex, doc_area: Rect) -> Rect {
		doc_area
	}
}
