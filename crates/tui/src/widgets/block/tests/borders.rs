//! Tests for border rendering and border types.

use rstest::rstest;

use super::*;

#[test]
fn render_padded_border() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
	Block::bordered()
		.border_type(BorderType::Padded)
		.render(buffer.area, &mut buffer);
	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"          ",
		"          ",
		"          ",
	]);
	assert_eq!(buffer, expected);
}

#[test]
fn render_stripe_border() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
	Block::bordered()
		.border_type(BorderType::Stripe)
		.render(buffer.area, &mut buffer);
	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"▏         ",
		"▏         ",
		"▏         ",
	]);
	assert_eq!(buffer, expected);
}

#[test]
fn render_border_types() {
	// Test all border type renderings in a single test using a helper
	fn check_border(border_type: BorderType, expected: [&str; 3]) {
		render_test!(
			Block::bordered().border_type(border_type),
			(10, 3),
			[expected[0], expected[1], expected[2]]
		);
	}

	check_border(
		BorderType::Plain,
		["┌────────┐", "│        │", "└────────┘"],
	);
	check_border(
		BorderType::Rounded,
		["╭────────╮", "│        │", "╰────────╯"],
	);
	check_border(
		BorderType::Double,
		["╔════════╗", "║        ║", "╚════════╝"],
	);
	check_border(
		BorderType::QuadrantInside,
		["▗▄▄▄▄▄▄▄▄▖", "▐        ▌", "▝▀▀▀▀▀▀▀▀▘"],
	);
	check_border(
		BorderType::QuadrantOutside,
		["▛▀▀▀▀▀▀▀▀▜", "▌        ▐", "▙▄▄▄▄▄▄▄▄▟"],
	);
	check_border(
		BorderType::Thick,
		["┏━━━━━━━━┓", "┃        ┃", "┗━━━━━━━━┛"],
	);
	check_border(
		BorderType::LightDoubleDashed,
		["┌╌╌╌╌╌╌╌╌┐", "╎        ╎", "└╌╌╌╌╌╌╌╌┘"],
	);
	check_border(
		BorderType::HeavyDoubleDashed,
		["┏╍╍╍╍╍╍╍╍┓", "╏        ╏", "┗╍╍╍╍╍╍╍╍┛"],
	);
	check_border(
		BorderType::LightTripleDashed,
		["┌┄┄┄┄┄┄┄┄┐", "┆        ┆", "└┄┄┄┄┄┄┄┄┘"],
	);
	check_border(
		BorderType::HeavyTripleDashed,
		["┏┅┅┅┅┅┅┅┅┓", "┇        ┇", "┗┅┅┅┅┅┅┅┅┛"],
	);
	check_border(
		BorderType::LightQuadrupleDashed,
		["┌┈┈┈┈┈┈┈┈┐", "┊        ┊", "└┈┈┈┈┈┈┈┈┘"],
	);
	check_border(
		BorderType::HeavyQuadrupleDashed,
		["┏┉┉┉┉┉┉┉┉┓", "┋        ┋", "┗┉┉┉┉┉┉┉┉┛"],
	);
}

#[test]
fn render_custom_border_set() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
	Block::bordered()
		.border_set(border::Set {
			top_left: "1",
			top_right: "2",
			bottom_left: "3",
			bottom_right: "4",
			vertical_left: "L",
			vertical_right: "R",
			horizontal_top: "T",
			horizontal_bottom: "B",
		})
		.render(buffer.area, &mut buffer);
	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"1TTTTTTTT2",
		"L        R",
		"3BBBBBBBB4",
	]);
	assert_eq!(buffer, expected);
}

#[rstest]
#[case::replace(MergeStrategy::Replace)]
#[case::exact(MergeStrategy::Exact)]
#[case::fuzzy(MergeStrategy::Fuzzy)]
fn render_partial_borders(#[case] strategy: MergeStrategy) {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
	Block::new()
		.border_type(BorderType::Plain)
		.borders(Borders::TOP | Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
		.merge_borders(strategy)
		.render(buffer.area, &mut buffer);
	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"┌────────┐",
		"│        │",
		"└────────┘",
	]);
	assert_eq!(buffer, expected);

	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
	Block::new()
		.border_type(BorderType::Plain)
		.borders(Borders::TOP | Borders::LEFT)
		.merge_borders(strategy)
		.render(buffer.area, &mut buffer);
	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"┌─────────",
		"│         ",
		"│         ",
	]);
	assert_eq!(buffer, expected);

	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
	Block::new()
		.border_type(BorderType::Plain)
		.borders(Borders::TOP | Borders::RIGHT)
		.merge_borders(strategy)
		.render(buffer.area, &mut buffer);
	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"─────────┐",
		"         │",
		"         │",
	]);
	assert_eq!(buffer, expected);

	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
	Block::new()
		.border_type(BorderType::Plain)
		.borders(Borders::BOTTOM | Borders::LEFT)
		.merge_borders(strategy)
		.render(buffer.area, &mut buffer);
	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"│         ",
		"│         ",
		"└─────────",
	]);
	assert_eq!(buffer, expected);

	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
	Block::new()
		.border_type(BorderType::Plain)
		.borders(Borders::BOTTOM | Borders::RIGHT)
		.merge_borders(strategy)
		.render(buffer.area, &mut buffer);
	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"         │",
		"         │",
		"─────────┘",
	]);
	assert_eq!(buffer, expected);

	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
	Block::new()
		.border_type(BorderType::Plain)
		.borders(Borders::TOP | Borders::BOTTOM)
		.merge_borders(strategy)
		.render(buffer.area, &mut buffer);
	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"──────────",
		"          ",
		"──────────",
	]);
	assert_eq!(buffer, expected);

	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
	Block::new()
		.border_type(BorderType::Plain)
		.borders(Borders::LEFT | Borders::RIGHT)
		.merge_borders(strategy)
		.render(buffer.area, &mut buffer);
	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"│        │",
		"│        │",
		"│        │",
	]);
	assert_eq!(buffer, expected);
}
