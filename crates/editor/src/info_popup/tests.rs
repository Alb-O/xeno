use super::*;

#[test]
fn measure_content_clamps_width_and_height() {
	let long_line = "x".repeat(80);
	let content = (0..30).map(|_| long_line.as_str()).collect::<Vec<_>>().join("\n");
	let (w, h) = measure_content(&content);
	assert_eq!(w, 60);
	assert_eq!(h, 20);
}

#[test]
fn store_next_id_is_monotonic() {
	let mut store = InfoPopupStore::default();
	assert_eq!(store.next_id().0, 0);
	assert_eq!(store.next_id().0, 1);
	assert_eq!(store.next_id().0, 2);
}

#[test]
fn store_render_plan_carries_popup_fields() {
	let mut store = InfoPopupStore::default();
	let id = store.next_id();
	store.insert(InfoPopup {
		id,
		buffer_id: ViewId(42),
		anchor: PopupAnchor::Point { x: 7, y: 9 },
		content_width: 48,
		content_height: 12,
	});

	let plan = store.render_plan();
	assert_eq!(plan.len(), 1);
	let target = plan[0];
	assert_eq!(target.id, id);
	assert_eq!(target.buffer_id, ViewId(42));
	assert_eq!(target.content_width, 48);
	assert_eq!(target.content_height, 12);
	match target.anchor {
		InfoPopupRenderAnchor::Point { x, y } => {
			assert_eq!(x, 7);
			assert_eq!(y, 9);
		}
		_ => panic!("expected point anchor"),
	}
}

#[test]
fn store_render_plan_maps_window_anchor_to_window() {
	let mut store = InfoPopupStore::default();
	let id = store.next_id();
	store.insert(InfoPopup {
		id,
		buffer_id: ViewId(7),
		anchor: PopupAnchor::Window(WindowId(3)),
		content_width: 20,
		content_height: 5,
	});

	let plan = store.render_plan();
	assert_eq!(plan.len(), 1);
	assert!(matches!(plan[0].anchor, InfoPopupRenderAnchor::Window(wid) if wid == WindowId(3)));
}

#[test]
fn store_render_plan_is_sorted_by_popup_id() {
	let mut store = InfoPopupStore::default();
	store.insert(InfoPopup {
		id: InfoPopupId(10),
		buffer_id: ViewId(1),
		anchor: PopupAnchor::Center,
		content_width: 10,
		content_height: 3,
	});
	store.insert(InfoPopup {
		id: InfoPopupId(2),
		buffer_id: ViewId(2),
		anchor: PopupAnchor::Center,
		content_width: 10,
		content_height: 3,
	});

	let plan = store.render_plan();
	assert_eq!(plan.len(), 2);
	assert_eq!(plan[0].id, InfoPopupId(2));
	assert_eq!(plan[1].id, InfoPopupId(10));
}

#[test]
fn popup_rect_centers_in_bounds() {
	let bounds = crate::geometry::Rect::new(0, 1, 80, 22);
	let rect = compute_popup_rect(InfoPopupRenderAnchor::Center, 20, 5, bounds, bounds).expect("rect should exist");
	assert!(rect.x > bounds.x);
	assert!(rect.y > bounds.y);
	assert!(rect.x + rect.width < bounds.x + bounds.width);
	assert!(rect.y + rect.height < bounds.y + bounds.height);
}

#[test]
fn popup_rect_clamps_point_to_bounds() {
	let bounds = crate::geometry::Rect::new(0, 1, 80, 22);
	let rect = compute_popup_rect(InfoPopupRenderAnchor::Point { x: 100, y: 100 }, 20, 5, bounds, bounds).expect("rect should exist");
	assert!(rect.x + rect.width <= bounds.x + bounds.width);
	assert!(rect.y + rect.height <= bounds.y + bounds.height);
}

#[test]
fn popup_rect_respects_point_position() {
	let bounds = crate::geometry::Rect::new(0, 1, 80, 22);
	let rect = compute_popup_rect(InfoPopupRenderAnchor::Point { x: 10, y: 5 }, 20, 5, bounds, bounds).expect("rect should exist");
	assert_eq!(rect.x, 10);
	assert_eq!(rect.y, 5);
}

#[test]
fn popup_rect_applies_content_caps() {
	let bounds = crate::geometry::Rect::new(0, 0, 200, 100);
	let rect = compute_popup_rect(InfoPopupRenderAnchor::Center, 120, 40, bounds, bounds).expect("rect should exist");
	assert_eq!(rect.width, 62);
	assert_eq!(rect.height, 14);
}

#[test]
fn popup_rect_window_anchor_centers_in_frame() {
	let bounds = crate::geometry::Rect::new(0, 0, 80, 24);
	// Window occupies the right half of the screen.
	let frame = crate::geometry::Rect::new(40, 0, 40, 24);
	let rect = compute_popup_rect(InfoPopupRenderAnchor::Window(WindowId(1)), 20, 5, frame, bounds).expect("rect should exist");
	// Popup should be centered within the frame (right half), not the full bounds.
	assert!(rect.x >= frame.x, "popup x={} should be >= frame x={}", rect.x, frame.x);
	let frame_center_x = frame.x + frame.width / 2;
	let popup_center_x = rect.x + rect.width / 2;
	// Popup center should be close to frame center (within a few cells of rounding).
	assert!((popup_center_x as i32 - frame_center_x as i32).unsigned_abs() <= 2);
}

#[test]
fn popup_rect_window_anchor_clamps_to_bounds() {
	let bounds = crate::geometry::Rect::new(0, 0, 80, 24);
	// Frame near the right edge — popup should still be clamped within bounds.
	let frame = crate::geometry::Rect::new(60, 0, 40, 24);
	let rect = compute_popup_rect(InfoPopupRenderAnchor::Window(WindowId(1)), 20, 5, frame, bounds).expect("rect should exist");
	assert!(rect.x + rect.width <= bounds.x + bounds.width, "popup should not escape bounds");
}

#[test]
fn inner_rect_matches_padding_policy() {
	let outer = crate::geometry::Rect::new(10, 5, 30, 8);
	let inner = popup_inner_rect(outer);
	assert_eq!(inner.x, 11);
	assert_eq!(inner.y, 5);
	assert_eq!(inner.width, 28);
	assert_eq!(inner.height, 8);
}

#[test]
fn inner_rect_saturates_narrow_popup() {
	let outer = crate::geometry::Rect::new(10, 5, 1, 4);
	let inner = popup_inner_rect(outer);
	assert_eq!(inner.width, 0);
}

#[test]
fn inner_rect_zero_width_popup() {
	let outer = crate::geometry::Rect::new(10, 5, 0, 4);
	let inner = popup_inner_rect(outer);
	assert_eq!(inner.width, 0);
}
