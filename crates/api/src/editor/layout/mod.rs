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
//! # Modules
//!
//! - [`manager`] - Core `LayoutManager` struct
//! - [`types`] - Type definitions (`LayerIndex`, `SeparatorId`, `SeparatorHit`)
//! - [`layers`] - Layer management and area computation
//! - [`views`] - View navigation and lookup
//! - [`splits`] - Split creation and removal
//! - [`separators`] - Separator hit detection
//! - [`drag`] - Drag state and hover animation

mod drag;
mod layers;
mod manager;
mod separators;
mod splits;
mod types;
mod views;

pub use manager::LayoutManager;
pub use types::{SeparatorHit, SeparatorId};

#[cfg(test)]
mod tests {
	use evildoer_registry::panels::PanelId;
	use evildoer_tui::layout::Rect;

	use super::*;
	use crate::buffer::{BufferId, BufferView, Layout};

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
