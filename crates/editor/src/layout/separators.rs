//! Separator detection and resizing.
//!
//! Finding separators at screen positions and resizing splits.
//!
//! # Generational Safety
//!
//! Separator IDs include generational [`LayerId`] to ensure stored references
//! don't access wrong layers after overlay reuse. See [`SeparatorId`] and
//! [`LayerId`] for details.

use super::manager::LayoutManager;
use super::types::{LayerError, LayerId, SeparatorHit, SeparatorId};
use crate::buffer::{Layout, SplitDirection};
use crate::geometry::Rect;

impl LayoutManager {
	/// Returns separator positions for rendering the base layer.
	pub fn separator_positions(&self, base_layout: &Layout, area: Rect) -> Vec<(SplitDirection, u8, Rect)> {
		base_layout.separator_positions(area)
	}

	/// Returns separator positions for a specific layer.
	pub fn separator_positions_for_layer(&self, base_layout: &Layout, layer: LayerId, area: Rect) -> Vec<(SplitDirection, u8, Rect)> {
		match self.layer(base_layout, layer) {
			Ok(l) => l.separator_positions(area),
			Err(_) => Vec::new(),
		}
	}

	/// Finds the separator at the given screen coordinates (searches top-down).
	///
	/// Returns the separator direction and rectangle. For interactive hit testing
	/// that needs a stable identifier, use [`Self::separator_hit_at_position`].
	pub fn separator_at_position(&self, base_layout: &Layout, area: Rect, x: u16, y: u16) -> Option<(SplitDirection, Rect)> {
		for i in (1..self.layers.len()).rev() {
			if let Some(ref layout) = self.layers[i].layout {
				let layer_id = LayerId::new(i as u16, self.layers[i].generation);
				let layer_area = self.layer_area(layer_id, area);
				if let Some(result) = layout.separator_at_position(layer_area, x, y) {
					return Some(result);
				}
			}
		}
		base_layout.separator_at_position(area, x, y)
	}

	/// Finds the separator at the given screen coordinates, building a hit record.
	///
	/// Searches separators top-down through layers, constructing a generational
	/// [`SeparatorId`] for stable references during interactions.
	pub fn separator_hit_at_position(&self, base_layout: &Layout, area: Rect, x: u16, y: u16) -> Option<SeparatorHit> {
		for i in (1..self.layers.len()).rev() {
			if let Some(ref layout) = self.layers[i].layout {
				let layer_id = LayerId::new(i as u16, self.layers[i].generation);
				let layer_area = self.layer_area(layer_id, area);
				if let Some((direction, rect, path)) = layout.separator_with_path_at_position(layer_area, x, y) {
					return Some(SeparatorHit {
						id: SeparatorId::Split { path, layer: layer_id },
						direction,
						rect,
					});
				}
			}
		}

		base_layout
			.separator_with_path_at_position(area, x, y)
			.map(|(direction, rect, path)| SeparatorHit {
				id: SeparatorId::Split { path, layer: LayerId::BASE },
				direction,
				rect,
			})
	}

	/// Validates that a separator ID is still valid.
	///
	/// # Errors
	///
	/// Returns [`LayerError`] if the layer has been cleared or reused.
	pub fn validate_separator_id(&self, id: &SeparatorId) -> Result<(), LayerError> {
		match id {
			SeparatorId::Split { layer, .. } => {
				self.validate_layer(*layer)?;
				Ok(())
			}
		}
	}

	/// Gets the rectangle for the given separator identifier.
	///
	/// Returns `None` if the layer is no longer valid or the path no longer exists.
	pub fn separator_rect(&self, base_layout: &Layout, area: Rect, id: &SeparatorId) -> Option<Rect> {
		match id {
			SeparatorId::Split { path, layer } => {
				let layer_area = self.layer_area(*layer, area);
				self.layer(base_layout, *layer)
					.ok()?
					.separator_rect_at_path(layer_area, path)
					.map(|(_, rect)| rect)
			}
		}
	}

	/// Resizes the separator identified by the given ID based on mouse position.
	///
	/// This method validates the identifier before resizing. If the ID is stale,
	/// the operation is ignored. Increments the layout revision on success.
	pub fn resize_separator(&mut self, base_layout: &mut Layout, area: Rect, id: &SeparatorId, mouse_x: u16, mouse_y: u16) {
		match id {
			SeparatorId::Split { path, layer } => {
				let layer_area = self.layer_area(*layer, area);
				if let Ok(layout) = self.layer_mut(base_layout, *layer) {
					layout.resize_at_path(layer_area, path, mouse_x, mouse_y);
					self.increment_revision();
				}
			}
		}
	}
}
