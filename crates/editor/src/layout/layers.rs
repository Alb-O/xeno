//! Layer management for stacked layouts.
//!
//! Layers are ordered from bottom (index 0) to top. Layer 0 is the base layout
//! owned by the base window. Higher layers overlay on top with transparent backgrounds.
//!
//! # Generational Layer IDs
//!
//! Overlay layers use generational tracking to prevent stale references.
//! See [`LayerId`] and [`LayerError`] for details.

use xeno_tui::layout::Rect;

use super::manager::LayoutManager;
use super::types::{LayerError, LayerId};
use crate::buffer::{Layout, ViewId};

impl LayoutManager {
	/// Validates a layer ID and returns the slot index if valid.
	///
	/// # Errors
	///
	/// - [`LayerError::InvalidIndex`] if the index is out of bounds for overlays.
	/// - [`LayerError::EmptyLayer`] if the slot exists but has no layout.
	/// - [`LayerError::StaleLayer`] if the generation doesn't match the current slot.
	pub(crate) fn validate_layer(&self, id: LayerId) -> Result<usize, LayerError> {
		if id.is_base() {
			return Ok(0);
		}

		let idx = id.index();
		if idx >= self.layers.len() {
			return Err(LayerError::InvalidIndex);
		}

		let slot = &self.layers[idx];
		if slot.layout.is_none() {
			return Err(LayerError::EmptyLayer);
		}
		if slot.generation != id.generation {
			return Err(LayerError::StaleLayer);
		}

		Ok(idx)
	}

	/// Returns a reference to the layout for a given layer ID.
	///
	/// # Errors
	///
	/// Returns [`LayerError`] if the ID is invalid, stale, or the layer is empty.
	pub fn layer<'a>(
		&'a self,
		base_layout: &'a Layout,
		id: LayerId,
	) -> Result<&'a Layout, LayerError> {
		if id.is_base() {
			return Ok(base_layout);
		}
		let idx = self.validate_layer(id)?;
		Ok(self.layers[idx].layout.as_ref().unwrap())
	}

	/// Returns a mutable reference to the layout for a given layer ID.
	///
	/// # Errors
	///
	/// Returns [`LayerError`] if the ID is invalid, stale, or the layer is empty.
	pub(crate) fn layer_mut<'a>(
		&'a mut self,
		base_layout: &'a mut Layout,
		id: LayerId,
	) -> Result<&'a mut Layout, LayerError> {
		if id.is_base() {
			return Ok(base_layout);
		}
		let idx = self.validate_layer(id)?;
		Ok(self.layers[idx].layout.as_mut().unwrap())
	}

	/// Executes a closure with a mutable reference to the layout for a given layer ID.
	///
	/// Bumps the layout revision after the closure completes to ensure stale
	/// interactions (like separator drags) are invalidated.
	///
	/// # Errors
	///
	/// Returns [`LayerError`] if the ID is invalid, stale, or the layer is empty.
	pub fn with_layer_mut<R>(
		&mut self,
		base_layout: &mut Layout,
		id: LayerId,
		f: impl FnOnce(&mut Layout) -> R,
	) -> Result<R, LayerError> {
		let layout = self.layer_mut(base_layout, id)?;
		let out = f(layout);
		self.increment_revision();
		Ok(out)
	}

	/// Sets the layout for a layer at the given index.
	///
	/// Creates intermediate empty slots if needed. Bumps the slot generation
	/// to invalidate any stored references and increments the layout revision.
	///
	/// Returns the [`LayerId`] for the set layer.
	///
	/// # Panics
	///
	/// Panics if `index == 0` (cannot set base layer via this method).
	pub fn set_layer(&mut self, index: usize, layout: Option<Layout>) -> LayerId {
		assert!(index != 0, "cannot set base layer via set_layer");

		while self.layers.len() <= index {
			self.layers.push(super::types::LayerSlot::empty());
		}

		let (new_gen, idx) = {
			let slot = &mut self.layers[index];
			slot.generation = slot.generation.wrapping_add(1);
			slot.layout = layout;
			(slot.generation, index as u16)
		};

		self.increment_revision();
		LayerId::new(idx, new_gen)
	}

	/// Returns the topmost non-empty layer identifier.
	///
	/// Returns [`LayerId::BASE`] if no overlay layers exist.
	pub fn top_layer(&self) -> LayerId {
		for i in (1..self.layers.len()).rev() {
			if self.layers[i].layout.is_some() {
				return LayerId::new(i as u16, self.layers[i].generation);
			}
		}
		LayerId::BASE
	}

	/// Returns the number of layer slots, including empty ones.
	pub fn layer_count(&self) -> usize {
		self.layers.len()
	}

	/// Finds which layer contains a specific view.
	///
	/// Returns [`LayerId::BASE`] if the view is in the base layout, otherwise
	/// returns the identifier for the overlay layer.
	pub fn layer_of_view(&self, base_layout: &Layout, view: ViewId) -> Option<LayerId> {
		if base_layout.contains_view(view) {
			return Some(LayerId::BASE);
		}
		self.layers
			.iter()
			.enumerate()
			.skip(1)
			.find(|(_, slot)| slot.layout.as_ref().is_some_and(|l| l.contains_view(view)))
			.map(|(i, slot)| LayerId::new(i as u16, slot.generation))
	}

	/// Computes the area for a specific layer.
	///
	/// Currently all layers occupy the full document area.
	pub fn layer_area(&self, _layer: LayerId, doc_area: Rect) -> Rect {
		doc_area
	}

	/// Returns `true` if a layer ID is valid and points to an existing layout.
	pub fn is_valid_layer(&self, id: LayerId) -> bool {
		self.validate_layer(id).is_ok()
	}

	/// Returns `true` if the layer slot at the given index contains a layout.
	///
	/// This is a low-level accessor for iterating over layer slots.
	/// Prefer using [`LayerId`] and [`Self::layer`] for most operations.
	pub(crate) fn layer_slot_has_layout(&self, index: usize) -> bool {
		index < self.layers.len() && self.layers[index].layout.is_some()
	}

	/// Returns the generation for a layer slot at the given index.
	///
	/// Returns 0 if the index is out of bounds.
	pub(crate) fn layer_slot_generation(&self, index: usize) -> u16 {
		self.layers.get(index).map(|s| s.generation).unwrap_or(0)
	}

	/// Returns a reference to the layout for an overlay layer.
	///
	/// This helper validates the identifier and returns the layout only for
	/// overlay layers. Returns `None` for the base layer.
	pub(crate) fn overlay_layout(&self, layer: LayerId) -> Option<&Layout> {
		if layer.is_base() {
			return None;
		}
		let idx = self.validate_layer(layer).ok()?;
		self.layers.get(idx)?.layout.as_ref()
	}
}
