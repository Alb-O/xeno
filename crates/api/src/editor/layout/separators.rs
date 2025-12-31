//! Separator detection and resizing.
//!
//! Finding separators at screen positions and resizing splits.

use evildoer_tui::layout::Rect;

use super::manager::LayoutManager;
use super::types::{LayerIndex, SeparatorHit, SeparatorId};
use crate::buffer::SplitDirection;

impl LayoutManager {
	/// Returns the separator rect between layer 0 and layer 1 (the bottom dock boundary).
	///
	/// Returns None if layer 1 is not visible.
	pub fn layer_boundary_separator(&self, doc_area: Rect) -> Option<Rect> {
		self.layer(1)?;
		let layer0_area = self.layer_area(0, doc_area);
		// Adjust width if side dock is visible
		let width = if self.layer(2).is_some() {
			doc_area
				.width
				.saturating_sub(self.effective_side_dock_width(doc_area.width))
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
}
