use super::*;
use crate::Range;

#[test]
fn single_selection() {
	let sel = Selection::single(5, 10);
	assert_eq!(sel.len(), 1);
	assert_eq!(sel.primary(), Range::new(5, 10));
}

#[test]
fn point_selection() {
	let sel = Selection::point(5);
	assert_eq!(sel.len(), 1);
	assert!(sel.primary().is_point());
}

#[test]
fn multi_selection() {
	let primary = Range::new(10, 15);
	let others = vec![Range::new(0, 5), Range::new(20, 25)];
	let sel = Selection::new(primary, others);
	assert_eq!(sel.len(), 3);
	assert_eq!(sel.primary(), Range::new(10, 15));
}

#[test]
fn merge_overlapping() {
	let primary = Range::new(0, 10);
	let others = vec![Range::new(5, 15)];
	let sel = Selection::new(primary, others);
	assert_eq!(sel.len(), 1);
	assert_eq!(sel.ranges()[0].min(), 0);
	assert_eq!(sel.ranges()[0].max(), 15);
}

#[test]
fn merge_duplicate_cursors() {
	let primary = Range::point(5);
	let others = vec![Range::point(5)];
	let sel = Selection::new(primary, others);
	assert_eq!(sel.len(), 1);
	assert_eq!(sel.primary(), Range::point(5));
}

#[test]
fn do_not_merge_near_adjacent() {
	let primary = Range::new(0, 5);
	let others = vec![Range::new(6, 10)];
	let sel = Selection::new(primary, others);
	assert_eq!(sel.len(), 2);
	assert_eq!(sel.ranges()[0].min(), 0);
	assert_eq!(sel.ranges()[0].max(), 5);
	assert_eq!(sel.ranges()[1].min(), 6);
	assert_eq!(sel.ranges()[1].max(), 10);
}

#[test]
fn no_merge_gap() {
	let primary = Range::new(0, 5);
	let others = vec![Range::new(6, 10)];
	let sel = Selection::new(primary, others);
	assert_eq!(sel.len(), 2);
}

#[test]
fn merge_overlaps_and_adjacent_command() {
	let primary = Range::new(0, 5);
	let others = vec![Range::new(5, 10), Range::new(12, 14)];
	let mut sel = Selection::new(primary, others);
	sel.merge_overlaps_and_adjacent();
	assert_eq!(sel.len(), 2);
	assert_eq!(sel.ranges()[0], Range::new(0, 10));
	assert_eq!(sel.ranges()[1], Range::new(12, 14));
}

#[test]
fn transform() {
	let sel = Selection::single(5, 10);
	let transformed = sel.transform(|r| Range::new(r.anchor + 1, r.head + 1));
	assert_eq!(transformed.primary(), Range::new(6, 11));
}

#[test]
fn contains() {
	let sel = Selection::single(5, 10);
	assert!(!sel.contains(4));
	assert!(sel.contains(5));
	assert!(sel.contains(7));
	assert!(sel.contains(10));
}
