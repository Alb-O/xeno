use super::*;

#[test]
fn popup_rect_centers_in_bounds() {
	let bounds = Rect::new(0, 1, 80, 22);
	let rect = compute_popup_rect(PopupAnchor::Center, 20, 5, bounds);
	assert!(rect.x > bounds.x);
	assert!(rect.y > bounds.y);
	assert!(rect.x + rect.width < bounds.x + bounds.width);
	assert!(rect.y + rect.height < bounds.y + bounds.height);
}

#[test]
fn popup_rect_clamps_point_to_bounds() {
	let bounds = Rect::new(0, 1, 80, 22);
	let rect = compute_popup_rect(PopupAnchor::Point { x: 100, y: 100 }, 20, 5, bounds);
	assert!(rect.x + rect.width <= bounds.x + bounds.width);
	assert!(rect.y + rect.height <= bounds.y + bounds.height);
}

#[test]
fn popup_rect_respects_point_position() {
	let bounds = Rect::new(0, 1, 80, 22);
	let rect = compute_popup_rect(PopupAnchor::Point { x: 10, y: 5 }, 20, 5, bounds);
	assert_eq!(rect.x, 10);
	assert_eq!(rect.y, 5);
}
