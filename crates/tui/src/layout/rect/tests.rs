use pretty_assertions::assert_eq;
use rstest::rstest;

use super::*;
use crate::layout::{Constraint, Layout};

#[test]
fn to_string() {
	assert_eq!(Rect::new(1, 2, 3, 4).to_string(), "3x4+1+2");
}

#[test]
fn new() {
	assert_eq!(
		Rect::new(1, 2, 3, 4),
		Rect {
			x: 1,
			y: 2,
			width: 3,
			height: 4
		}
	);
}

#[test]
fn area() {
	assert_eq!(Rect::new(1, 2, 3, 4).area(), 12);
}

#[test]
fn is_empty() {
	assert!(!Rect::new(1, 2, 3, 4).is_empty());
	assert!(Rect::new(1, 2, 0, 4).is_empty());
	assert!(Rect::new(1, 2, 3, 0).is_empty());
}

#[test]
fn left() {
	assert_eq!(Rect::new(1, 2, 3, 4).left(), 1);
}

#[test]
fn right() {
	assert_eq!(Rect::new(1, 2, 3, 4).right(), 4);
}

#[test]
fn top() {
	assert_eq!(Rect::new(1, 2, 3, 4).top(), 2);
}

#[test]
fn bottom() {
	assert_eq!(Rect::new(1, 2, 3, 4).bottom(), 6);
}

#[test]
fn inner() {
	assert_eq!(
		Rect::new(1, 2, 3, 4).inner(Margin::new(1, 2)),
		Rect::new(2, 4, 1, 0)
	);
}

#[test]
fn outer() {
	// enough space to grow on all sides
	assert_eq!(
		Rect::new(100, 200, 10, 20).outer(Margin::new(20, 30)),
		Rect::new(80, 170, 50, 80)
	);

	// left / top saturation should truncate the size (10 less on left / top)
	assert_eq!(
		Rect::new(10, 20, 10, 20).outer(Margin::new(20, 30)),
		Rect::new(0, 0, 40, 70),
	);

	// right / bottom saturation should truncate the size (10 less on bottom / right)
	assert_eq!(
		Rect::new(u16::MAX - 20, u16::MAX - 40, 10, 20).outer(Margin::new(20, 30)),
		Rect::new(u16::MAX - 40, u16::MAX - 70, 40, 70),
	);
}

#[test]
fn offset() {
	assert_eq!(
		Rect::new(1, 2, 3, 4).offset(Offset { x: 5, y: 6 }),
		Rect::new(6, 8, 3, 4),
	);
}

#[test]
fn negative_offset() {
	assert_eq!(
		Rect::new(4, 3, 3, 4).offset(Offset { x: -2, y: -1 }),
		Rect::new(2, 2, 3, 4),
	);
}

#[test]
fn negative_offset_saturate() {
	assert_eq!(
		Rect::new(1, 2, 3, 4).offset(Offset { x: -5, y: -6 }),
		Rect::new(0, 0, 3, 4),
	);
}

/// Offsets a [`Rect`] making it go outside [`u16::MAX`], it should keep its size.
#[test]
fn offset_saturate_max() {
	assert_eq!(
		Rect::new(u16::MAX - 500, u16::MAX - 500, 100, 100).offset(Offset { x: 1000, y: 1000 }),
		Rect::new(u16::MAX - 100, u16::MAX - 100, 100, 100),
	);
}

#[test]
fn union() {
	assert_eq!(
		Rect::new(1, 2, 3, 4).union(Rect::new(2, 3, 4, 5)),
		Rect::new(1, 2, 5, 6)
	);
}

#[test]
fn intersection() {
	assert_eq!(
		Rect::new(1, 2, 3, 4).intersection(Rect::new(2, 3, 4, 5)),
		Rect::new(2, 3, 2, 3)
	);
}

#[test]
fn intersection_underflow() {
	assert_eq!(
		Rect::new(1, 1, 2, 2).intersection(Rect::new(4, 4, 2, 2)),
		Rect::new(4, 4, 0, 0)
	);
}

#[test]
fn intersects() {
	assert!(Rect::new(1, 2, 3, 4).intersects(Rect::new(2, 3, 4, 5)));
	assert!(!Rect::new(1, 2, 3, 4).intersects(Rect::new(5, 6, 7, 8)));
}

#[rstest]
#[case::corner(Rect::new(0, 0, 10, 10), Rect::new(10, 10, 20, 20))]
#[case::edge(Rect::new(0, 0, 10, 10), Rect::new(10, 0, 20, 10))]
#[case::no_intersect(Rect::new(0, 0, 10, 10), Rect::new(11, 11, 20, 20))]
#[case::contains(Rect::new(0, 0, 20, 20), Rect::new(5, 5, 10, 10))]
fn mutual_intersect(#[case] rect0: Rect, #[case] rect1: Rect) {
	assert_eq!(rect0.intersection(rect1), rect1.intersection(rect0));
	assert_eq!(rect0.intersects(rect1), rect1.intersects(rect0));
}

// the bounds of this rect are x: [1..=3], y: [2..=5]
#[rstest]
#[case::inside_top_left(Rect::new(1, 2, 3, 4), Position { x: 1, y: 2 }, true)]
#[case::inside_top_right(Rect::new(1, 2, 3, 4), Position { x: 3, y: 2 }, true)]
#[case::inside_bottom_left(Rect::new(1, 2, 3, 4), Position { x: 1, y: 5 }, true)]
#[case::inside_bottom_right(Rect::new(1, 2, 3, 4), Position { x: 3, y: 5 }, true)]
#[case::outside_left(Rect::new(1, 2, 3, 4), Position { x: 0, y: 2 }, false)]
#[case::outside_right(Rect::new(1, 2, 3, 4), Position { x: 4, y: 2 }, false)]
#[case::outside_top(Rect::new(1, 2, 3, 4), Position { x: 1, y: 1 }, false)]
#[case::outside_bottom(Rect::new(1, 2, 3, 4), Position { x: 1, y: 6 }, false)]
#[case::outside_top_left(Rect::new(1, 2, 3, 4), Position { x: 0, y: 1 }, false)]
#[case::outside_bottom_right(Rect::new(1, 2, 3, 4), Position { x: 4, y: 6 }, false)]
fn contains(#[case] rect: Rect, #[case] position: Position, #[case] expected: bool) {
	assert_eq!(
		rect.contains(position),
		expected,
		"rect: {rect:?}, position: {position:?}",
	);
}

#[test]
fn size_truncation() {
	assert_eq!(
		Rect::new(u16::MAX - 100, u16::MAX - 1000, 200, 2000),
		Rect {
			x: u16::MAX - 100,
			y: u16::MAX - 1000,
			width: 100,
			height: 1000
		}
	);
}

#[test]
fn size_preservation() {
	assert_eq!(
		Rect::new(u16::MAX - 100, u16::MAX - 1000, 100, 1000),
		Rect {
			x: u16::MAX - 100,
			y: u16::MAX - 1000,
			width: 100,
			height: 1000
		}
	);
}

#[test]
fn resize_updates_size() {
	let rect = Rect::new(10, 20, 5, 5).resize(Size::new(30, 40));
	assert_eq!(rect, Rect::new(10, 20, 30, 40));
}

#[test]
fn resize_clamps_at_bounds() {
	let rect = Rect::new(u16::MAX - 2, u16::MAX - 3, 1, 1).resize(Size::new(10, 10));
	assert_eq!(rect, Rect::new(u16::MAX - 2, u16::MAX - 3, 2, 3));
}

#[test]
fn can_be_const() {
	const RECT: Rect = Rect {
		x: 0,
		y: 0,
		width: 10,
		height: 10,
	};
	const _AREA: u32 = RECT.area();
	const _LEFT: u16 = RECT.left();
	const _RIGHT: u16 = RECT.right();
	const _TOP: u16 = RECT.top();
	const _BOTTOM: u16 = RECT.bottom();
	assert!(RECT.intersects(RECT));
}

#[test]
fn split() {
	let [a, b] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
		.areas(Rect::new(0, 0, 2, 1));
	assert_eq!(a, Rect::new(0, 0, 1, 1));
	assert_eq!(b, Rect::new(1, 0, 1, 1));
}

#[test]
#[should_panic(expected = "expected 3 rects, got 2")]
fn split_invalid_number_of_recs() {
	let layout = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]);
	let [_a, _b, _c] = layout.areas(Rect::new(0, 0, 2, 1));
}

#[rstest]
#[case::inside(Rect::new(20, 20, 10, 10), Rect::new(20, 20, 10, 10))]
#[case::up_left(Rect::new(5, 5, 10, 10), Rect::new(10, 10, 10, 10))]
#[case::up(Rect::new(20, 5, 10, 10), Rect::new(20, 10, 10, 10))]
#[case::up_right(Rect::new(105, 5, 10, 10), Rect::new(100, 10, 10, 10))]
#[case::left(Rect::new(5, 20, 10, 10), Rect::new(10, 20, 10, 10))]
#[case::right(Rect::new(105, 20, 10, 10), Rect::new(100, 20, 10, 10))]
#[case::down_left(Rect::new(5, 105, 10, 10), Rect::new(10, 100, 10, 10))]
#[case::down(Rect::new(20, 105, 10, 10), Rect::new(20, 100, 10, 10))]
#[case::down_right(Rect::new(105, 105, 10, 10), Rect::new(100, 100, 10, 10))]
#[case::too_wide(Rect::new(5, 20, 200, 10), Rect::new(10, 20, 100, 10))]
#[case::too_tall(Rect::new(20, 5, 10, 200), Rect::new(20, 10, 10, 100))]
#[case::too_large(Rect::new(0, 0, 200, 200), Rect::new(10, 10, 100, 100))]
fn clamp(#[case] rect: Rect, #[case] expected: Rect) {
	let other = Rect::new(10, 10, 100, 100);
	assert_eq!(rect.clamp(other), expected);
}

#[test]
fn rows() {
	let area = Rect::new(0, 0, 3, 2);
	let rows: Vec<Rect> = area.rows().collect();

	let expected_rows: Vec<Rect> = vec![Rect::new(0, 0, 3, 1), Rect::new(0, 1, 3, 1)];

	assert_eq!(rows, expected_rows);
}

#[test]
fn columns() {
	let area = Rect::new(0, 0, 3, 2);
	let columns: Vec<Rect> = area.columns().collect();

	let expected_columns: Vec<Rect> = vec![
		Rect::new(0, 0, 1, 2),
		Rect::new(1, 0, 1, 2),
		Rect::new(2, 0, 1, 2),
	];

	assert_eq!(columns, expected_columns);
}

#[test]
fn as_position() {
	let rect = Rect::new(1, 2, 3, 4);
	let position = rect.as_position();
	assert_eq!(position.x, 1);
	assert_eq!(position.y, 2);
}

#[test]
fn as_size() {
	assert_eq!(
		Rect::new(1, 2, 3, 4).as_size(),
		Size {
			width: 3,
			height: 4
		}
	);
}

#[test]
fn from_position_and_size() {
	let position = Position { x: 1, y: 2 };
	let size = Size {
		width: 3,
		height: 4,
	};
	assert_eq!(
		Rect::from((position, size)),
		Rect {
			x: 1,
			y: 2,
			width: 3,
			height: 4
		}
	);
}

#[test]
fn from_size() {
	let size = Size {
		width: 3,
		height: 4,
	};
	assert_eq!(
		Rect::from(size),
		Rect {
			x: 0,
			y: 0,
			width: 3,
			height: 4
		}
	);
}

#[test]
fn centered_horizontally() {
	let rect = Rect::new(0, 0, 5, 5);
	assert_eq!(
		rect.centered_horizontally(Constraint::Length(3)),
		Rect::new(1, 0, 3, 5)
	);
}

#[test]
fn centered_vertically() {
	let rect = Rect::new(0, 0, 5, 5);
	assert_eq!(
		rect.centered_vertically(Constraint::Length(1)),
		Rect::new(0, 2, 5, 1)
	);
}

#[test]
fn centered() {
	let rect = Rect::new(0, 0, 5, 5);
	assert_eq!(
		rect.centered(Constraint::Length(3), Constraint::Length(1)),
		Rect::new(1, 2, 3, 1)
	);
}

#[test]
fn layout() {
	let layout = Layout::horizontal([Constraint::Length(3), Constraint::Min(0)]);

	let [a, b] = Rect::new(0, 0, 10, 10).layout(&layout);
	assert_eq!(a, Rect::new(0, 0, 3, 10));
	assert_eq!(b, Rect::new(3, 0, 7, 10));

	let areas = Rect::new(0, 0, 10, 10).layout::<2>(&layout);
	assert_eq!(areas[0], Rect::new(0, 0, 3, 10));
	assert_eq!(areas[1], Rect::new(3, 0, 7, 10));
}

#[test]
#[should_panic(expected = "invalid number of rects: expected 3, found 1")]
fn layout_invalid_number_of_rects() {
	let layout = Layout::horizontal([Constraint::Length(1)]);
	let [_, _, _] = Rect::new(0, 0, 10, 10).layout(&layout);
}

#[test]
fn layout_vec() {
	let layout = Layout::horizontal([Constraint::Length(3), Constraint::Min(0)]);

	let areas = Rect::new(0, 0, 10, 10).layout_vec(&layout);
	assert_eq!(areas[0], Rect::new(0, 0, 3, 10));
	assert_eq!(areas[1], Rect::new(3, 0, 7, 10));
}

#[test]
fn try_layout() {
	let layout = Layout::horizontal([Constraint::Length(3), Constraint::Min(0)]);

	let [a, b] = Rect::new(0, 0, 10, 10).try_layout(&layout).unwrap();
	assert_eq!(a, Rect::new(0, 0, 3, 10));
	assert_eq!(b, Rect::new(3, 0, 7, 10));

	let areas = Rect::new(0, 0, 10, 10).try_layout::<2>(&layout).unwrap();
	assert_eq!(areas[0], Rect::new(0, 0, 3, 10));
	assert_eq!(areas[1], Rect::new(3, 0, 7, 10));
}

#[test]
fn try_layout_invalid_number_of_rects() {
	let layout = Layout::horizontal([Constraint::Length(1)]);
	Rect::new(0, 0, 10, 10)
		.try_layout::<3>(&layout)
		.unwrap_err();
}
