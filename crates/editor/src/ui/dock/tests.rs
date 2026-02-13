use super::{DockManager, DockSlot, SizeSpec};
use crate::geometry::Rect;

#[test]
fn bottom_slot_defaults_to_fixed_lines() {
	let dock = DockManager::new();
	let bottom = dock.slots.get(&DockSlot::Bottom).expect("bottom slot should exist");
	assert_eq!(bottom.size, SizeSpec::Lines(10));
}

#[test]
fn fixed_bottom_height_reduces_doc_area_deterministically() {
	let mut dock = DockManager::new();
	dock.open_panel(DockSlot::Bottom, "utility".to_string());

	let area = Rect::new(0, 0, 100, 40);
	let layout = dock.compute_layout(area);

	assert_eq!(layout.doc_area.height, 30);
	assert_eq!(layout.doc_area.y, 0);
	assert_eq!(layout.panel_areas.get("utility").map(|r| r.height), Some(10));
}

#[test]
fn fixed_bottom_height_clamps_under_tiny_viewports() {
	let mut dock = DockManager::new();
	dock.open_panel(DockSlot::Bottom, "utility".to_string());

	let area = Rect::new(0, 0, 80, 8);
	let layout = dock.compute_layout(area);

	assert_eq!(layout.doc_area.height, 0);
	assert_eq!(layout.panel_areas.get("utility").map(|r| r.height), Some(8));
}
