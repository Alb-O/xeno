//! Tests for Buffer indexing and cell access.

use rstest::rstest;

use super::*;

#[test]
fn it_translates_to_and_from_coordinates() {
	let rect = Rect::new(200, 100, 50, 80);
	let buf = Buffer::empty(rect);

	// First cell is at the upper left corner.
	assert_eq!(buf.pos_of(0), (200, 100));
	assert_eq!(buf.index_of(200, 100), 0);

	// Last cell is in the lower right.
	assert_eq!(buf.pos_of(buf.content.len() - 1), (249, 179));
	assert_eq!(buf.index_of(249, 179), buf.content.len() - 1);
}

#[test]
#[should_panic(expected = "outside the buffer")]
fn pos_of_panics_on_out_of_bounds() {
	let rect = Rect::new(0, 0, 10, 10);
	let buf = Buffer::empty(rect);

	// There are a total of 100 cells; zero-indexed means that 100 would be the 101st cell.
	let _ = buf.pos_of(100);
}

#[rstest]
#[case::left(9, 10)]
#[case::top(10, 9)]
#[case::right(20, 10)]
#[case::bottom(10, 20)]
#[should_panic(expected = "index outside of buffer: the area is Rect { x: 10, y: 10, width: 10, height: 10 } but index is")]
fn index_of_panics_on_out_of_bounds(#[case] x: u16, #[case] y: u16) {
	let _ = Buffer::empty(Rect::new(10, 10, 10, 10)).index_of(x, y);
}

#[test]
fn test_cell() {
	let buf = Buffer::with_lines(["Hello", "World"]);

	let mut expected = Cell::default();
	expected.set_symbol("H");

	assert_eq!(buf.cell((0, 0)), Some(&expected));
	assert_eq!(buf.cell((10, 10)), None);
	assert_eq!(buf.cell(Position::new(0, 0)), Some(&expected));
	assert_eq!(buf.cell(Position::new(10, 10)), None);
}

#[test]
fn test_cell_mut() {
	let mut buf = Buffer::with_lines(["Hello", "World"]);

	let mut expected = Cell::default();
	expected.set_symbol("H");

	assert_eq!(buf.cell_mut((0, 0)), Some(&mut expected));
	assert_eq!(buf.cell_mut((10, 10)), None);
	assert_eq!(buf.cell_mut(Position::new(0, 0)), Some(&mut expected));
	assert_eq!(buf.cell_mut(Position::new(10, 10)), None);
}

#[test]
fn index() {
	let buf = Buffer::with_lines(["Hello", "World"]);

	let mut expected = Cell::default();
	expected.set_symbol("H");

	assert_eq!(buf[(0, 0)], expected);
}

#[rstest]
#[case::left(9, 10)]
#[case::top(10, 9)]
#[case::right(20, 10)]
#[case::bottom(10, 20)]
#[should_panic(expected = "index outside of buffer: the area is Rect { x: 10, y: 10, width: 10, height: 10 } but index is")]
fn index_out_of_bounds_panics(#[case] x: u16, #[case] y: u16) {
	let rect = Rect::new(10, 10, 10, 10);
	let buf = Buffer::empty(rect);
	let _ = buf[(x, y)];
}

#[test]
fn index_mut() {
	let mut buf = Buffer::with_lines(["Cat", "Dog"]);
	buf[(0, 0)].set_symbol("B");
	buf[Position::new(0, 1)].set_symbol("L");
	assert_eq!(buf, Buffer::with_lines(["Bat", "Log"]));
}

#[rstest]
#[case::left(9, 10)]
#[case::top(10, 9)]
#[case::right(20, 10)]
#[case::bottom(10, 20)]
#[should_panic(expected = "index outside of buffer: the area is Rect { x: 10, y: 10, width: 10, height: 10 } but index is")]
fn index_mut_out_of_bounds_panics(#[case] x: u16, #[case] y: u16) {
	let mut buf = Buffer::empty(Rect::new(10, 10, 10, 10));
	buf[(x, y)].set_symbol("A");
}

/// Regression test for
///
/// Previously the `pos_of` function would incorrectly cast the index to a u16 value instead of
/// using the index as is. This caused incorrect rendering of any buffer with an length > 65535.
#[test]
fn index_pos_of_u16_max() {
	let buffer = Buffer::empty(Rect::new(0, 0, 256, 256 + 1));
	assert_eq!(buffer.index_of(255, 255), 65535);
	assert_eq!(buffer.pos_of(65535), (255, 255));

	assert_eq!(buffer.index_of(0, 256), 65536);
	assert_eq!(buffer.pos_of(65536), (0, 256)); // previously (0, 0)

	assert_eq!(buffer.index_of(1, 256), 65537);
	assert_eq!(buffer.pos_of(65537), (1, 256)); // previously (1, 0)

	assert_eq!(buffer.index_of(255, 256), 65791);
	assert_eq!(buffer.pos_of(65791), (255, 256)); // previously (255, 0)
}
