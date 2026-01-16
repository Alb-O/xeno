//! Separator detection and resizing.
//!
//! Finding separators at screen positions and resizing splits.

use xeno_tui::layout::Rect;

use super::manager::LayoutManager;
use super::types::{LayerIndex, SeparatorHit, SeparatorId};
use crate::buffer::{Layout, SplitDirection};

impl LayoutManager {
	/// Returns separator positions for rendering (base layer).
	pub fn separator_positions(
		&self,
		base_layout: &Layout,
		area: Rect,
	) -> Vec<(SplitDirection, u8, Rect)> {
		base_layout.separator_positions(area)
	}

	/// Returns separator positions for a specific layer.
	pub fn separator_positions_for_layer(
		&self,
		base_layout: &Layout,
		layer: LayerIndex,
		area: Rect,
	) -> Vec<(SplitDirection, u8, Rect)> {
		self.layer(base_layout, layer)
			.map(|l| l.separator_positions(area))
			.unwrap_or_default()
	}

	/// Finds the separator at the given screen coordinates (searches top-down).
	pub fn separator_at_position(
		&self,
		base_layout: &Layout,
		area: Rect,
		x: u16,
		y: u16,
	) -> Option<(SplitDirection, Rect)> {
		for i in (1..self.layers.len()).rev() {
			if let Some(layout) = &self.layers[i] {
				let layer_area = self.layer_area(i, area);
				if let Some(result) = layout.separator_at_position(layer_area, x, y) {
					return Some(result);
				}
			}
		}
		base_layout.separator_at_position(area, x, y)
	}

	/// Finds the separator at the given screen coordinates.
	///
	/// Searches split separators top-down through layers.
	pub fn separator_hit_at_position(
		&self,
		base_layout: &Layout,
		area: Rect,
		x: u16,
		y: u16,
	) -> Option<SeparatorHit> {
		for i in (1..self.layers.len()).rev() {
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
		base_layout
			.separator_with_path_at_position(area, x, y)
			.map(|(direction, rect, path)| SeparatorHit {
				id: SeparatorId::Split { path, layer: 0 },
				direction,
				rect,
			})
	}

	/// Gets the separator rect for the given separator ID.
	pub fn separator_rect(
		&self,
		base_layout: &Layout,
		area: Rect,
		id: &SeparatorId,
	) -> Option<Rect> {
		match id {
			SeparatorId::Split { path, layer } => {
				let layer_area = self.layer_area(*layer, area);
				self.layer(base_layout, *layer)?
					.separator_rect_at_path(layer_area, path)
					.map(|(_, rect)| rect)
			}
		}
	}

	/// Resizes the separator identified by the given ID based on mouse position.
	pub fn resize_separator(
		&mut self,
		base_layout: &mut Layout,
		area: Rect,
		id: &SeparatorId,
		mouse_x: u16,
		mouse_y: u16,
	) {
		match id {
			SeparatorId::Split { path, layer } => {
				let layer_area = self.layer_area(*layer, area);
				if let Some(layout) = self.layer_mut(base_layout, *layer) {
					layout.resize_at_path(layer_area, path, mouse_x, mouse_y);
				}
			}
		}
	}
}
