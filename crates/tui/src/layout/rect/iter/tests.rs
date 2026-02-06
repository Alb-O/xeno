use super::*;

#[test]
fn rows() {
	let rect = Rect::new(0, 0, 2, 3);
	let mut rows = Rows::new(rect);
	assert_eq!(rows.size_hint(), (3, Some(3)));
	assert_eq!(rows.next(), Some(Rect::new(0, 0, 2, 1)));
	assert_eq!(rows.size_hint(), (2, Some(2)));
	assert_eq!(rows.next(), Some(Rect::new(0, 1, 2, 1)));
	assert_eq!(rows.size_hint(), (1, Some(1)));
	assert_eq!(rows.next(), Some(Rect::new(0, 2, 2, 1)));
	assert_eq!(rows.size_hint(), (0, Some(0)));
	assert_eq!(rows.next(), None);
	assert_eq!(rows.size_hint(), (0, Some(0)));
	assert_eq!(rows.next_back(), None);
	assert_eq!(rows.size_hint(), (0, Some(0)));
}

#[test]
fn rows_back() {
	let rect = Rect::new(0, 0, 2, 3);
	let mut rows = Rows::new(rect);
	assert_eq!(rows.size_hint(), (3, Some(3)));
	assert_eq!(rows.next_back(), Some(Rect::new(0, 2, 2, 1)));
	assert_eq!(rows.size_hint(), (2, Some(2)));
	assert_eq!(rows.next_back(), Some(Rect::new(0, 1, 2, 1)));
	assert_eq!(rows.size_hint(), (1, Some(1)));
	assert_eq!(rows.next_back(), Some(Rect::new(0, 0, 2, 1)));
	assert_eq!(rows.size_hint(), (0, Some(0)));
	assert_eq!(rows.next_back(), None);
	assert_eq!(rows.size_hint(), (0, Some(0)));
	assert_eq!(rows.next(), None);
	assert_eq!(rows.size_hint(), (0, Some(0)));
}

#[test]
fn rows_meet_in_the_middle() {
	let rect = Rect::new(0, 0, 2, 4);
	let mut rows = Rows::new(rect);
	assert_eq!(rows.size_hint(), (4, Some(4)));
	assert_eq!(rows.next(), Some(Rect::new(0, 0, 2, 1)));
	assert_eq!(rows.size_hint(), (3, Some(3)));
	assert_eq!(rows.next_back(), Some(Rect::new(0, 3, 2, 1)));
	assert_eq!(rows.size_hint(), (2, Some(2)));
	assert_eq!(rows.next(), Some(Rect::new(0, 1, 2, 1)));
	assert_eq!(rows.size_hint(), (1, Some(1)));
	assert_eq!(rows.next_back(), Some(Rect::new(0, 2, 2, 1)));
	assert_eq!(rows.size_hint(), (0, Some(0)));
	assert_eq!(rows.next(), None);
	assert_eq!(rows.size_hint(), (0, Some(0)));
	assert_eq!(rows.next_back(), None);
	assert_eq!(rows.size_hint(), (0, Some(0)));
}

#[test]
fn columns() {
	let rect = Rect::new(0, 0, 3, 2);
	let mut columns = Columns::new(rect);
	assert_eq!(columns.size_hint(), (3, Some(3)));
	assert_eq!(columns.next(), Some(Rect::new(0, 0, 1, 2)));
	assert_eq!(columns.size_hint(), (2, Some(2)));
	assert_eq!(columns.next(), Some(Rect::new(1, 0, 1, 2)));
	assert_eq!(columns.size_hint(), (1, Some(1)));
	assert_eq!(columns.next(), Some(Rect::new(2, 0, 1, 2)));
	assert_eq!(columns.size_hint(), (0, Some(0)));
	assert_eq!(columns.next(), None);
	assert_eq!(columns.size_hint(), (0, Some(0)));
	assert_eq!(columns.next_back(), None);
	assert_eq!(columns.size_hint(), (0, Some(0)));
}

#[test]
fn columns_back() {
	let rect = Rect::new(0, 0, 3, 2);
	let mut columns = Columns::new(rect);
	assert_eq!(columns.size_hint(), (3, Some(3)));
	assert_eq!(columns.next_back(), Some(Rect::new(2, 0, 1, 2)));
	assert_eq!(columns.size_hint(), (2, Some(2)));
	assert_eq!(columns.next_back(), Some(Rect::new(1, 0, 1, 2)));
	assert_eq!(columns.size_hint(), (1, Some(1)));
	assert_eq!(columns.next_back(), Some(Rect::new(0, 0, 1, 2)));
	assert_eq!(columns.size_hint(), (0, Some(0)));
	assert_eq!(columns.next_back(), None);
	assert_eq!(columns.size_hint(), (0, Some(0)));
	assert_eq!(columns.next(), None);
	assert_eq!(columns.size_hint(), (0, Some(0)));
}

#[test]
fn columns_meet_in_the_middle() {
	let rect = Rect::new(0, 0, 4, 2);
	let mut columns = Columns::new(rect);
	assert_eq!(columns.size_hint(), (4, Some(4)));
	assert_eq!(columns.next(), Some(Rect::new(0, 0, 1, 2)));
	assert_eq!(columns.size_hint(), (3, Some(3)));
	assert_eq!(columns.next_back(), Some(Rect::new(3, 0, 1, 2)));
	assert_eq!(columns.size_hint(), (2, Some(2)));
	assert_eq!(columns.next(), Some(Rect::new(1, 0, 1, 2)));
	assert_eq!(columns.size_hint(), (1, Some(1)));
	assert_eq!(columns.next_back(), Some(Rect::new(2, 0, 1, 2)));
	assert_eq!(columns.size_hint(), (0, Some(0)));
	assert_eq!(columns.next(), None);
	assert_eq!(columns.size_hint(), (0, Some(0)));
	assert_eq!(columns.next_back(), None);
	assert_eq!(columns.size_hint(), (0, Some(0)));
}

/// We allow a total of `65536` columns in the range `(0..=65535)`.  In this test we iterate
/// forward and skip the first `65534` columns, and expect the next column to be `65535` and
/// the subsequent columns to be `None`.
#[test]
fn columns_max() {
	let rect = Rect::new(0, 0, u16::MAX, 1);
	let mut columns = Columns::new(rect).skip(usize::from(u16::MAX - 1));
	assert_eq!(columns.next(), Some(Rect::new(u16::MAX - 1, 0, 1, 1)));
	assert_eq!(columns.next(), None);
}

/// We allow a total of `65536` columns in the range `(0..=65535)`.  In this test we iterate
/// backward and skip the last `65534` columns, and expect the next column to be `0` and the
/// subsequent columns to be `None`.
#[test]
fn columns_min() {
	let rect = Rect::new(0, 0, u16::MAX, 1);
	let mut columns = Columns::new(rect).rev().skip(usize::from(u16::MAX - 1));
	assert_eq!(columns.next(), Some(Rect::new(0, 0, 1, 1)));
	assert_eq!(columns.next(), None);
	assert_eq!(columns.next(), None);
}

#[test]
fn positions() {
	let rect = Rect::new(0, 0, 2, 2);
	let mut positions = Positions::new(rect);
	assert_eq!(positions.size_hint(), (4, Some(4)));
	assert_eq!(positions.next(), Some(Position::new(0, 0)));
	assert_eq!(positions.size_hint(), (3, Some(3)));
	assert_eq!(positions.next(), Some(Position::new(1, 0)));
	assert_eq!(positions.size_hint(), (2, Some(2)));
	assert_eq!(positions.next(), Some(Position::new(0, 1)));
	assert_eq!(positions.size_hint(), (1, Some(1)));
	assert_eq!(positions.next(), Some(Position::new(1, 1)));
	assert_eq!(positions.size_hint(), (0, Some(0)));
	assert_eq!(positions.next(), None);
	assert_eq!(positions.size_hint(), (0, Some(0)));
}

#[test]
fn positions_zero_width() {
	let rect = Rect::new(0, 0, 0, 1);
	let mut positions = Positions::new(rect);
	assert_eq!(positions.size_hint(), (0, Some(0)));
	assert_eq!(positions.next(), None);
	assert_eq!(positions.size_hint(), (0, Some(0)));
}

#[test]
fn positions_zero_height() {
	let rect = Rect::new(0, 0, 1, 0);
	let mut positions = Positions::new(rect);
	assert_eq!(positions.size_hint(), (0, Some(0)));
	assert_eq!(positions.next(), None);
	assert_eq!(positions.size_hint(), (0, Some(0)));
}

#[test]
fn positions_zero_by_zero() {
	let rect = Rect::new(0, 0, 0, 0);
	let mut positions = Positions::new(rect);
	assert_eq!(positions.size_hint(), (0, Some(0)));
	assert_eq!(positions.next(), None);
	assert_eq!(positions.size_hint(), (0, Some(0)));
}
