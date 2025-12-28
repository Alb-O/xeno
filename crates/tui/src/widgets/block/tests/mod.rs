use alloc::vec;

use super::*;
use crate::style::{Color, Modifier, Stylize};
use crate::{enum_display_from_str_tests, render_test};

mod borders;
mod inner;
mod merge;
mod styling;
mod titles;

#[test]
fn create_with_all_borders() {
	let block = Block::bordered();
	assert_eq!(block.borders, Borders::all());
}

#[test]
fn block_new() {
	assert_eq!(
		Block::new(),
		Block {
			titles: Vec::new(),
			titles_style: Style::new(),
			titles_alignment: Alignment::Left,
			titles_position: TitlePosition::Top,
			borders: Borders::NONE,
			border_style: Style::new(),
			border_set: BorderType::Padded.to_border_set(),
			style: Style::new(),
			padding: Padding::ZERO,
			merge_borders: MergeStrategy::Replace,
		}
	);
}

#[test]
const fn block_can_be_const() {
	const _DEFAULT_STYLE: Style = Style::new();
	const _DEFAULT_PADDING: Padding = Padding::uniform(1);
	const _DEFAULT_BLOCK: Block = Block::bordered()
		// the following methods are no longer const because they use Into<Style>
		// .style(_DEFAULT_STYLE)           // no longer const
		// .border_style(_DEFAULT_STYLE)    // no longer const
		// .title_style(_DEFAULT_STYLE)     // no longer const
		.title_alignment(Alignment::Left)
		.title_position(TitlePosition::Top)
		.padding(_DEFAULT_PADDING);
}

#[test]
const fn border_type_can_be_const() {
	const _PLAIN: border::Set = BorderType::border_symbols(BorderType::Plain);
}

#[test]
fn border_type_display_and_from_str() {
	enum_display_from_str_tests!(
		BorderType,
		[
			Plain,
			Rounded,
			Double,
			Thick,
			Padded,
			Stripe,
			QuadrantInside,
			QuadrantOutside,
			LightDoubleDashed,
			HeavyDoubleDashed,
			LightTripleDashed,
			HeavyTripleDashed,
			LightQuadrupleDashed,
			HeavyQuadrupleDashed,
		]
	);
}

#[test]
fn render_in_minimal_buffer() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
	// This should not panic, even if the buffer is too small to render the block.
	Block::bordered()
		.border_type(BorderType::Plain)
		.title("I'm too big for this buffer")
		.padding(Padding::uniform(10))
		.render(buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(["┌"]));
}

#[test]
fn render_in_zero_size_buffer() {
	let mut buffer = Buffer::empty(Rect::ZERO);
	// This should not panic, even if the buffer has zero size.
	Block::bordered()
		.title("I'm too big for this buffer")
		.padding(Padding::uniform(10))
		.render(buffer.area, &mut buffer);
}
