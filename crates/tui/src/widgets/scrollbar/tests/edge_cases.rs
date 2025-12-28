//! Edge case tests for scrollbar rendering

use rstest::rstest;

use super::*;

#[rstest]
#[case::scrollbar_height_0(10, 0)]
#[case::scrollbar_width_0(0, 10)]
fn do_not_render_with_empty_area(#[case] width: u16, #[case] height: u16) {
	let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
		.begin_symbol(Some("<"))
		.end_symbol(Some(">"))
		.track_symbol(Some("-"))
		.thumb_symbol("#");
	let zero_width_area = Rect::new(0, 0, width, height);
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 10));

	let mut state = ScrollbarState::new(10);
	scrollbar.render(zero_width_area, &mut buffer, &mut state);
}

#[rstest]
#[case::vertical_left(ScrollbarOrientation::VerticalLeft)]
#[case::vertical_right(ScrollbarOrientation::VerticalRight)]
#[case::horizontal_top(ScrollbarOrientation::HorizontalTop)]
#[case::horizontal_bottom(ScrollbarOrientation::HorizontalBottom)]
fn render_in_minimal_buffer(#[case] orientation: ScrollbarOrientation) {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
	let scrollbar = Scrollbar::new(orientation);
	let mut state = ScrollbarState::new(10).position(5);
	// This should not panic, even if the buffer is too small to render the scrollbar.
	scrollbar.render(buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines([" "]));
}

#[rstest]
#[case::vertical_left(ScrollbarOrientation::VerticalLeft)]
#[case::vertical_right(ScrollbarOrientation::VerticalRight)]
#[case::horizontal_top(ScrollbarOrientation::HorizontalTop)]
#[case::horizontal_bottom(ScrollbarOrientation::HorizontalBottom)]
fn render_in_zero_size_buffer(#[case] orientation: ScrollbarOrientation) {
	let mut buffer = Buffer::empty(Rect::ZERO);
	let scrollbar = Scrollbar::new(orientation);
	let mut state = ScrollbarState::new(10).position(5);
	// This should not panic, even if the buffer has zero size.
	scrollbar.render(buffer.area, &mut buffer, &mut state);
}
