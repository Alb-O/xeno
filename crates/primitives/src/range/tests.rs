use ropey::Rope;

use super::*;

#[test]
fn range_basics() {
	let r = Range::new(5, 10);
	assert_eq!(r.min(), 5);
	assert_eq!(r.max(), 10);
	assert_eq!(r.len(), 6);
	assert!(!r.is_point());
	assert_eq!(r.direction(), Direction::Forward);
}

#[test]
fn range_backward() {
	let r = Range::new(10, 5);
	assert_eq!(r.min(), 5);
	assert_eq!(r.max(), 10);
	assert_eq!(r.direction(), Direction::Backward);
}

#[test]
fn range_from_to_forward() {
	// Forward selection: anchor=5, head=10
	// Selects characters 5,6,7,8,9,10 (head position is included)
	let r = Range::new(5, 10);
	assert_eq!(r.from(), 5);
	assert_eq!(r.to(), 11);
	assert_eq!(r.len(), 6);
}

#[test]
fn range_from_to_backward() {
	// Backward selection: anchor=10, head=5
	// Selects characters 5,6,7,8,9,10 (anchor char IS selected)
	let r = Range::new(10, 5);
	assert_eq!(r.from(), 5);
	assert_eq!(r.to(), 11);
	assert_eq!(r.len(), 6);
}

#[test]
fn range_flip() {
	let r = Range::new(5, 10);
	let flipped = r.flip();
	assert_eq!(flipped.anchor, 10);
	assert_eq!(flipped.head, 5);
}

#[test]
fn range_point() {
	let r = Range::point(5);
	assert_eq!(r.len(), 1);
	assert!(r.is_point());
	assert_eq!(r.anchor, 5);
	assert_eq!(r.head, 5);
}

#[test]
fn range_contains() {
	let r = Range::new(5, 10);
	assert!(!r.contains(4));
	assert!(r.contains(5));
	assert!(r.contains(7));
	assert!(r.contains(10));
}

#[test]
fn range_overlaps() {
	let r1 = Range::new(5, 10);
	let r2 = Range::new(8, 15);
	let r3 = Range::new(10, 15);

	assert!(r1.overlaps(&r2));
	assert!(r1.overlaps(&r3)); // Overlap at character 10
}

#[test]
fn range_overlaps_same_point() {
	let r1 = Range::point(5);
	let r2 = Range::point(5);

	assert!(r1.overlaps(&r2));
}

#[test]
fn range_merge() {
	let r1 = Range::new(5, 10);
	let r2 = Range::new(8, 15);
	let merged = r1.merge(&r2);
	assert_eq!(merged.min(), 5);
	assert_eq!(merged.max(), 15);
}

#[test]
fn grapheme_aligned() {
	let text = Rope::from("hello");
	let slice = text.slice(..);
	let r = Range::new(1, 3);
	let aligned = r.grapheme_aligned(slice);
	assert_eq!(aligned.anchor, 1);
	assert_eq!(aligned.head, 3);
}
