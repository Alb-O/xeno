use super::{Position, Rect};

#[test]
fn new_rect_saturates_dimensions() {
	let rect = Rect::new(u16::MAX - 1, u16::MAX - 1, 10, 10);
	assert_eq!(rect.width, 1);
	assert_eq!(rect.height, 1);
}

#[test]
fn rect_edges_are_exclusive() {
	let rect = Rect::new(10, 5, 3, 2);
	assert_eq!(rect.left(), 10);
	assert_eq!(rect.right(), 13);
	assert_eq!(rect.top(), 5);
	assert_eq!(rect.bottom(), 7);
}

#[test]
fn contains_uses_inclusive_origin_exclusive_max() {
	let rect = Rect::new(10, 5, 3, 2);
	assert!(rect.contains(Position::new(10, 5)));
	assert!(rect.contains(Position::new(12, 6)));
	assert!(!rect.contains(Position::new(13, 6)));
	assert!(!rect.contains(Position::new(12, 7)));
}
