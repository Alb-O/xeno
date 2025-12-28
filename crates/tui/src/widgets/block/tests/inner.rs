//! Tests for inner(), vertical_space(), and horizontal_space() calculations.

use rstest::rstest;

use super::*;

#[rstest]
#[case::none_0(Borders::NONE, Rect::ZERO, Rect::ZERO)]
#[case::none_1(Borders::NONE, Rect::new(0, 0, 1, 1), Rect::new(0, 0, 1, 1))]
#[case::left_0(Borders::LEFT, Rect::ZERO, Rect::ZERO)]
#[case::left_w1(Borders::LEFT, Rect::new(0, 0, 0, 1), Rect::new(0, 0, 0, 1))]
#[case::left_w2(Borders::LEFT, Rect::new(0, 0, 1, 1), Rect::new(1, 0, 0, 1))]
#[case::left_w3(Borders::LEFT, Rect::new(0, 0, 2, 1), Rect::new(1, 0, 1, 1))]
#[case::top_0(Borders::TOP, Rect::ZERO, Rect::ZERO)]
#[case::top_h1(Borders::TOP, Rect::new(0, 0, 1, 0), Rect::new(0, 0, 1, 0))]
#[case::top_h2(Borders::TOP, Rect::new(0, 0, 1, 1), Rect::new(0, 1, 1, 0))]
#[case::top_h3(Borders::TOP, Rect::new(0, 0, 1, 2), Rect::new(0, 1, 1, 1))]
#[case::right_0(Borders::RIGHT, Rect::ZERO, Rect::ZERO)]
#[case::right_w1(Borders::RIGHT, Rect::new(0, 0, 0, 1), Rect::new(0, 0, 0, 1))]
#[case::right_w2(Borders::RIGHT, Rect::new(0, 0, 1, 1), Rect::new(0, 0, 0, 1))]
#[case::right_w3(Borders::RIGHT, Rect::new(0, 0, 2, 1), Rect::new(0, 0, 1, 1))]
#[case::bottom_0(Borders::BOTTOM, Rect::ZERO, Rect::ZERO)]
#[case::bottom_h1(Borders::BOTTOM, Rect::new(0, 0, 1, 0), Rect::new(0, 0, 1, 0))]
#[case::bottom_h2(Borders::BOTTOM, Rect::new(0, 0, 1, 1), Rect::new(0, 0, 1, 0))]
#[case::bottom_h3(Borders::BOTTOM, Rect::new(0, 0, 1, 2), Rect::new(0, 0, 1, 1))]
#[case::all_0(Borders::ALL, Rect::ZERO, Rect::ZERO)]
#[case::all_1(Borders::ALL, Rect::new(0, 0, 1, 1), Rect::new(1, 1, 0, 0))]
#[case::all_2(Borders::ALL, Rect::new(0, 0, 2, 2), Rect::new(1, 1, 0, 0))]
#[case::all_3(Borders::ALL, Rect::new(0, 0, 3, 3), Rect::new(1, 1, 1, 1))]
fn inner_takes_into_account_the_borders(
	#[case] borders: Borders,
	#[case] area: Rect,
	#[case] expected: Rect,
) {
	let block = Block::new().borders(borders);
	assert_eq!(block.inner(area), expected);
}

#[rstest]
#[case::left(Alignment::Left)]
#[case::center(Alignment::Center)]
#[case::right(Alignment::Right)]
fn inner_takes_into_account_the_title(#[case] alignment: Alignment) {
	let area = Rect::new(0, 0, 0, 1);
	let expected = Rect::new(0, 1, 0, 0);

	let block = Block::new().title(Line::from("Test").alignment(alignment));
	assert_eq!(block.inner(area), expected);
}

#[rstest]
#[case::top_top(Block::new().title_top("Test").borders(Borders::TOP), Rect::new(0, 1, 0, 1))]
#[case::top_bot(Block::new().title_top("Test").borders(Borders::BOTTOM), Rect::new(0, 1, 0, 0))]
#[case::bot_top(Block::new().title_bottom("Test").borders(Borders::TOP), Rect::new(0, 1, 0, 0))]
#[case::bot_bot(Block::new().title_bottom("Test").borders(Borders::BOTTOM), Rect::new(0, 0, 0, 1))]
fn inner_takes_into_account_border_and_title(#[case] block: Block, #[case] expected: Rect) {
	let area = Rect::new(0, 0, 0, 2);
	assert_eq!(block.inner(area), expected);
}

#[test]
fn has_title_at_position_takes_into_account_all_positioning_declarations() {
	let block = Block::new();
	assert!(!block.has_title_at_position(TitlePosition::Top));
	assert!(!block.has_title_at_position(TitlePosition::Bottom));

	let block = Block::new().title_top("test");
	assert!(block.has_title_at_position(TitlePosition::Top));
	assert!(!block.has_title_at_position(TitlePosition::Bottom));

	let block = Block::new().title_bottom("test");
	assert!(!block.has_title_at_position(TitlePosition::Top));
	assert!(block.has_title_at_position(TitlePosition::Bottom));

	let block = Block::new().title_top("test").title_bottom("test");
	assert!(block.has_title_at_position(TitlePosition::Top));
	assert!(block.has_title_at_position(TitlePosition::Bottom));
}

#[rstest]
#[case::none(Borders::NONE, (0, 0))]
#[case::top(Borders::TOP, (1, 0))]
#[case::right(Borders::RIGHT, (0, 0))]
#[case::bottom(Borders::BOTTOM, (0, 1))]
#[case::left(Borders::LEFT, (0, 0))]
#[case::top_right(Borders::TOP | Borders::RIGHT, (1, 0))]
#[case::top_bottom(Borders::TOP | Borders::BOTTOM, (1, 1))]
#[case::top_left(Borders::TOP | Borders::LEFT, (1, 0))]
#[case::bottom_right(Borders::BOTTOM | Borders::RIGHT, (0, 1))]
#[case::bottom_left(Borders::BOTTOM | Borders::LEFT, (0, 1))]
#[case::left_right(Borders::LEFT | Borders::RIGHT, (0, 0))]
fn vertical_space_takes_into_account_borders(
	#[case] borders: Borders,
	#[case] vertical_space: (u16, u16),
) {
	let block = Block::new().borders(borders);
	assert_eq!(block.vertical_space(), vertical_space);
}

#[rstest]
#[case::top_border_top_p1(Borders::TOP, Padding::new(0, 0, 1, 0), (2, 0))]
#[case::right_border_top_p1(Borders::RIGHT, Padding::new(0, 0, 1, 0), (1, 0))]
#[case::bottom_border_top_p1(Borders::BOTTOM, Padding::new(0, 0, 1, 0), (1, 1))]
#[case::left_border_top_p1(Borders::LEFT, Padding::new(0, 0, 1, 0), (1, 0))]
#[case::top_bottom_border_all_p3(Borders::TOP | Borders::BOTTOM, Padding::new(100, 100, 4, 5), (5, 6))]
#[case::no_border(Borders::NONE, Padding::new(100, 100, 10, 13), (10, 13))]
#[case::all(Borders::ALL, Padding::new(100, 100, 1, 3), (2, 4))]
fn vertical_space_takes_into_account_padding(
	#[case] borders: Borders,
	#[case] padding: Padding,
	#[case] vertical_space: (u16, u16),
) {
	let block = Block::new().borders(borders).padding(padding);
	assert_eq!(block.vertical_space(), vertical_space);
}

#[test]
fn vertical_space_takes_into_account_titles() {
	let block = Block::new().title_top("Test");
	assert_eq!(block.vertical_space(), (1, 0));

	let block = Block::new().title_bottom("Test");
	assert_eq!(block.vertical_space(), (0, 1));
}

#[rstest]
#[case::top_border_top_title(Block::new(), Borders::TOP, TitlePosition::Top, (1, 0))]
#[case::right_border_top_title(Block::new(), Borders::RIGHT, TitlePosition::Top, (1, 0))]
#[case::bottom_border_top_title(Block::new(), Borders::BOTTOM, TitlePosition::Top, (1, 1))]
#[case::left_border_top_title(Block::new(), Borders::LEFT, TitlePosition::Top, (1, 0))]
#[case::top_border_top_title(Block::new(), Borders::TOP, TitlePosition::Bottom, (1, 1))]
#[case::right_border_top_title(Block::new(), Borders::RIGHT, TitlePosition::Bottom, (0, 1))]
#[case::bottom_border_top_title(Block::new(), Borders::BOTTOM, TitlePosition::Bottom, (0, 1))]
#[case::left_border_top_title(Block::new(), Borders::LEFT, TitlePosition::Bottom, (0, 1))]
fn vertical_space_takes_into_account_borders_and_title(
	#[case] block: Block,
	#[case] borders: Borders,
	#[case] pos: TitlePosition,
	#[case] vertical_space: (u16, u16),
) {
	let block = block.borders(borders).title_position(pos).title("Test");
	assert_eq!(block.vertical_space(), vertical_space);
}

#[test]
fn horizontal_space_takes_into_account_borders() {
	let block = Block::bordered();
	assert_eq!(block.horizontal_space(), (1, 1));

	let block = Block::new().borders(Borders::LEFT);
	assert_eq!(block.horizontal_space(), (1, 0));

	let block = Block::new().borders(Borders::RIGHT);
	assert_eq!(block.horizontal_space(), (0, 1));
}

#[test]
fn horizontal_space_takes_into_account_padding() {
	let block = Block::new().padding(Padding::new(1, 1, 100, 100));
	assert_eq!(block.horizontal_space(), (1, 1));

	let block = Block::new().padding(Padding::new(3, 5, 0, 0));
	assert_eq!(block.horizontal_space(), (3, 5));

	let block = Block::new().padding(Padding::new(0, 1, 100, 100));
	assert_eq!(block.horizontal_space(), (0, 1));

	let block = Block::new().padding(Padding::new(1, 0, 100, 100));
	assert_eq!(block.horizontal_space(), (1, 0));
}

#[rstest]
#[case::all_bordered_all_padded(Block::bordered(), Padding::new(1, 1, 1, 1), (2, 2))]
#[case::all_bordered_left_padded(Block::bordered(), Padding::new(1, 0, 0, 0), (2, 1))]
#[case::all_bordered_right_padded(Block::bordered(), Padding::new(0, 1, 0, 0), (1, 2))]
#[case::all_bordered_top_padded(Block::bordered(), Padding::new(0, 0, 1, 0), (1, 1))]
#[case::all_bordered_bottom_padded(Block::bordered(), Padding::new(0, 0, 0, 1), (1, 1))]
#[case::left_bordered_left_padded(Block::new().borders(Borders::LEFT), Padding::new(1, 0, 0, 0), (2, 0))]
#[case::left_bordered_right_padded(Block::new().borders(Borders::LEFT), Padding::new(0, 1, 0, 0), (1, 1))]
#[case::right_bordered_right_padded(Block::new().borders(Borders::RIGHT), Padding::new(0, 1, 0, 0), (0, 2))]
#[case::right_bordered_left_padded(Block::new().borders(Borders::RIGHT), Padding::new(1, 0, 0, 0), (1, 1))]
fn horizontal_space_takes_into_account_borders_and_padding(
	#[case] block: Block,
	#[case] padding: Padding,
	#[case] horizontal_space: (u16, u16),
) {
	let block = block.padding(padding);
	assert_eq!(block.horizontal_space(), horizontal_space);
}
