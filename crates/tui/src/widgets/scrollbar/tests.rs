use alloc::format;
use alloc::string::ToString;
use core::str::FromStr;

use rstest::{fixture, rstest};
use strum::ParseError;

use super::*;
use crate::text::Text;
use crate::widgets::Widget;

#[test]
fn scroll_direction_to_string() {
	assert_eq!(ScrollDirection::Forward.to_string(), "Forward");
	assert_eq!(ScrollDirection::Backward.to_string(), "Backward");
}

#[test]
fn scroll_direction_from_str() {
	assert_eq!("Forward".parse(), Ok(ScrollDirection::Forward));
	assert_eq!("Backward".parse(), Ok(ScrollDirection::Backward));
	assert_eq!(
		ScrollDirection::from_str(""),
		Err(ParseError::VariantNotFound)
	);
}

#[test]
fn scrollbar_orientation_to_string() {
	use ScrollbarOrientation::*;
	assert_eq!(VerticalRight.to_string(), "VerticalRight");
	assert_eq!(VerticalLeft.to_string(), "VerticalLeft");
	assert_eq!(HorizontalBottom.to_string(), "HorizontalBottom");
	assert_eq!(HorizontalTop.to_string(), "HorizontalTop");
}

#[test]
fn scrollbar_orientation_from_str() {
	use ScrollbarOrientation::*;
	assert_eq!("VerticalRight".parse(), Ok(VerticalRight));
	assert_eq!("VerticalLeft".parse(), Ok(VerticalLeft));
	assert_eq!("HorizontalBottom".parse(), Ok(HorizontalBottom));
	assert_eq!("HorizontalTop".parse(), Ok(HorizontalTop));
	assert_eq!(
		ScrollbarOrientation::from_str(""),
		Err(ParseError::VariantNotFound)
	);
}

#[fixture]
fn scrollbar_no_arrows() -> Scrollbar<'static> {
	Scrollbar::new(ScrollbarOrientation::HorizontalTop)
		.begin_symbol(None)
		.end_symbol(None)
		.track_symbol(Some("-"))
		.thumb_symbol("#")
}

#[rstest]
#[case::area_2_position_0("#-", 0, 2)]
#[case::area_2_position_1("-#", 1, 2)]
fn render_scrollbar_simplest(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
	scrollbar_no_arrows: Scrollbar,
) {
	let mut buffer = Buffer::empty(Rect::new(0, 0, expected.width() as u16, 1));
	let mut state = ScrollbarState::new(content_length).position(position);
	scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines([expected]));
}

#[rstest]
#[case::position_0("#####-----", 0, 10)]
#[case::position_1("-#####----", 1, 10)]
#[case::position_2("-#####----", 2, 10)]
#[case::position_3("--#####---", 3, 10)]
#[case::position_4("--#####---", 4, 10)]
#[case::position_5("---#####--", 5, 10)]
#[case::position_6("---#####--", 6, 10)]
#[case::position_7("----#####-", 7, 10)]
#[case::position_8("----#####-", 8, 10)]
#[case::position_9("-----#####", 9, 10)]
fn render_scrollbar_simple(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
	scrollbar_no_arrows: Scrollbar,
) {
	let mut buffer = Buffer::empty(Rect::new(0, 0, expected.width() as u16, 1));
	let mut state = ScrollbarState::new(content_length).position(position);
	scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines([expected]));
}

#[rstest]
#[case::position_0("          ", 0, 0)]
fn render_scrollbar_nobar(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
	scrollbar_no_arrows: Scrollbar,
) {
	let size = expected.width();
	let mut buffer = Buffer::empty(Rect::new(0, 0, size as u16, 1));
	let mut state = ScrollbarState::new(content_length).position(position);
	scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines([expected]));
}

#[rstest]
#[case::fullbar_position_0("##########", 0, 1)]
#[case::almost_fullbar_position_0("#########-", 0, 2)]
#[case::almost_fullbar_position_1("-#########", 1, 2)]
fn render_scrollbar_fullbar(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
	scrollbar_no_arrows: Scrollbar,
) {
	let size = expected.width();
	let mut buffer = Buffer::empty(Rect::new(0, 0, size as u16, 1));
	let mut state = ScrollbarState::new(content_length).position(position);
	scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines([expected]));
}

#[rstest]
#[case::position_0("#########-", 0, 2)]
#[case::position_1("-#########", 1, 2)]
fn render_scrollbar_almost_fullbar(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
	scrollbar_no_arrows: Scrollbar,
) {
	let size = expected.width();
	let mut buffer = Buffer::empty(Rect::new(0, 0, size as u16, 1));
	let mut state = ScrollbarState::new(content_length).position(position);
	scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines([expected]));
}

#[rstest]
#[case::position_0("█████═════", 0, 10)]
#[case::position_1("═█████════", 1, 10)]
#[case::position_2("═█████════", 2, 10)]
#[case::position_3("══█████═══", 3, 10)]
#[case::position_4("══█████═══", 4, 10)]
#[case::position_5("═══█████══", 5, 10)]
#[case::position_6("═══█████══", 6, 10)]
#[case::position_7("════█████═", 7, 10)]
#[case::position_8("════█████═", 8, 10)]
#[case::position_9("═════█████", 9, 10)]
#[case::position_out_of_bounds("═════█████", 100, 10)]
fn render_scrollbar_without_symbols(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
) {
	let size = expected.width() as u16;
	let mut buffer = Buffer::empty(Rect::new(0, 0, size, 1));
	let mut state = ScrollbarState::new(content_length).position(position);
	Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
		.begin_symbol(None)
		.end_symbol(None)
		.render(buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines([expected]));
}

#[rstest]
#[case::position_0("█████     ", 0, 10)]
#[case::position_1(" █████    ", 1, 10)]
#[case::position_2(" █████    ", 2, 10)]
#[case::position_3("  █████   ", 3, 10)]
#[case::position_4("  █████   ", 4, 10)]
#[case::position_5("   █████  ", 5, 10)]
#[case::position_6("   █████  ", 6, 10)]
#[case::position_7("    █████ ", 7, 10)]
#[case::position_8("    █████ ", 8, 10)]
#[case::position_9("     █████", 9, 10)]
#[case::position_out_of_bounds("     █████", 100, 10)]
fn render_scrollbar_without_track_symbols(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
) {
	let size = expected.width() as u16;
	let mut buffer = Buffer::empty(Rect::new(0, 0, size, 1));
	let mut state = ScrollbarState::new(content_length).position(position);
	Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
		.track_symbol(None)
		.begin_symbol(None)
		.end_symbol(None)
		.render(buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines([expected]));
}

#[rstest]
#[case::position_0("█████-----", 0, 10)]
#[case::position_1("-█████----", 1, 10)]
#[case::position_2("-█████----", 2, 10)]
#[case::position_3("--█████---", 3, 10)]
#[case::position_4("--█████---", 4, 10)]
#[case::position_5("---█████--", 5, 10)]
#[case::position_6("---█████--", 6, 10)]
#[case::position_7("----█████-", 7, 10)]
#[case::position_8("----█████-", 8, 10)]
#[case::position_9("-----█████", 9, 10)]
#[case::position_out_of_bounds("-----█████", 100, 10)]
fn render_scrollbar_without_track_symbols_over_content(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
) {
	let size = expected.width() as u16;
	let mut buffer = Buffer::empty(Rect::new(0, 0, size, 1));
	let width = buffer.area.width as usize;
	let s = "";
	Text::from(format!("{s:-^width$}")).render(buffer.area, &mut buffer);
	let mut state = ScrollbarState::new(content_length).position(position);
	Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
		.track_symbol(None)
		.begin_symbol(None)
		.end_symbol(None)
		.render(buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines([expected]));
}

#[rstest]
#[case::position_0("<####---->", 0, 10)]
#[case::position_1("<#####--->", 1, 10)]
#[case::position_2("<-####--->", 2, 10)]
#[case::position_3("<-####--->", 3, 10)]
#[case::position_4("<--####-->", 4, 10)]
#[case::position_5("<--####-->", 5, 10)]
#[case::position_6("<---####->", 6, 10)]
#[case::position_7("<---####->", 7, 10)]
#[case::position_8("<---#####>", 8, 10)]
#[case::position_9("<----####>", 9, 10)]
#[case::position_one_out_of_bounds("<----####>", 10, 10)]
#[case::position_few_out_of_bounds("<----####>", 15, 10)]
#[case::position_very_many_out_of_bounds("<----####>", 500, 10)]
fn render_scrollbar_with_symbols(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
) {
	let size = expected.width() as u16;
	let mut buffer = Buffer::empty(Rect::new(0, 0, size, 1));
	let mut state = ScrollbarState::new(content_length).position(position);
	Scrollbar::new(ScrollbarOrientation::HorizontalTop)
		.begin_symbol(Some("<"))
		.end_symbol(Some(">"))
		.track_symbol(Some("-"))
		.thumb_symbol("#")
		.render(buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines([expected]));
}

#[rstest]
#[case::position_0("█████═════", 0, 10)]
#[case::position_1("═█████════", 1, 10)]
#[case::position_2("═█████════", 2, 10)]
#[case::position_3("══█████═══", 3, 10)]
#[case::position_4("══█████═══", 4, 10)]
#[case::position_5("═══█████══", 5, 10)]
#[case::position_6("═══█████══", 6, 10)]
#[case::position_7("════█████═", 7, 10)]
#[case::position_8("════█████═", 8, 10)]
#[case::position_9("═════█████", 9, 10)]
#[case::position_out_of_bounds("═════█████", 100, 10)]
fn render_scrollbar_horizontal_bottom(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
) {
	let size = expected.width() as u16;
	let mut buffer = Buffer::empty(Rect::new(0, 0, size, 2));
	let mut state = ScrollbarState::new(content_length).position(position);
	Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
		.begin_symbol(None)
		.end_symbol(None)
		.render(buffer.area, &mut buffer, &mut state);
	let empty_string = " ".repeat(size as usize);
	assert_eq!(buffer, Buffer::with_lines([&empty_string, expected]));
}

#[rstest]
#[case::position_0("█████═════", 0, 10)]
#[case::position_1("═█████════", 1, 10)]
#[case::position_2("═█████════", 2, 10)]
#[case::position_3("══█████═══", 3, 10)]
#[case::position_4("══█████═══", 4, 10)]
#[case::position_5("═══█████══", 5, 10)]
#[case::position_6("═══█████══", 6, 10)]
#[case::position_7("════█████═", 7, 10)]
#[case::position_8("════█████═", 8, 10)]
#[case::position_9("═════█████", 9, 10)]
#[case::position_out_of_bounds("═════█████", 100, 10)]
fn render_scrollbar_horizontal_top(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
) {
	let size = expected.width() as u16;
	let mut buffer = Buffer::empty(Rect::new(0, 0, size, 2));
	let mut state = ScrollbarState::new(content_length).position(position);
	Scrollbar::new(ScrollbarOrientation::HorizontalTop)
		.begin_symbol(None)
		.end_symbol(None)
		.render(buffer.area, &mut buffer, &mut state);
	let empty_string = " ".repeat(size as usize);
	assert_eq!(buffer, Buffer::with_lines([expected, &empty_string]));
}

#[rstest]
#[case::position_0("<####---->", 0, 10)]
#[case::position_1("<#####--->", 1, 10)]
#[case::position_2("<-####--->", 2, 10)]
#[case::position_3("<-####--->", 3, 10)]
#[case::position_4("<--####-->", 4, 10)]
#[case::position_5("<--####-->", 5, 10)]
#[case::position_6("<---####->", 6, 10)]
#[case::position_7("<---####->", 7, 10)]
#[case::position_8("<---#####>", 8, 10)]
#[case::position_9("<----####>", 9, 10)]
#[case::position_one_out_of_bounds("<----####>", 10, 10)]
#[case::position_few_out_of_bounds("<----####>", 15, 10)]
#[case::position_very_many_out_of_bounds("<----####>", 500, 10)]
fn render_scrollbar_vertical_left(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
) {
	let size = expected.width() as u16;
	let mut buffer = Buffer::empty(Rect::new(0, 0, 5, size));
	let mut state = ScrollbarState::new(content_length).position(position);
	Scrollbar::new(ScrollbarOrientation::VerticalLeft)
		.begin_symbol(Some("<"))
		.end_symbol(Some(">"))
		.track_symbol(Some("-"))
		.thumb_symbol("#")
		.render(buffer.area, &mut buffer, &mut state);
	let bar = expected.chars().map(|c| format!("{c}    "));
	assert_eq!(buffer, Buffer::with_lines(bar));
}

#[rstest]
#[case::position_0("<####---->", 0, 10)]
#[case::position_1("<#####--->", 1, 10)]
#[case::position_2("<-####--->", 2, 10)]
#[case::position_3("<-####--->", 3, 10)]
#[case::position_4("<--####-->", 4, 10)]
#[case::position_5("<--####-->", 5, 10)]
#[case::position_6("<---####->", 6, 10)]
#[case::position_7("<---####->", 7, 10)]
#[case::position_8("<---#####>", 8, 10)]
#[case::position_9("<----####>", 9, 10)]
#[case::position_one_out_of_bounds("<----####>", 10, 10)]
#[case::position_few_out_of_bounds("<----####>", 15, 10)]
#[case::position_very_many_out_of_bounds("<----####>", 500, 10)]
fn render_scrollbar_vertical_right(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
) {
	let size = expected.width() as u16;
	let mut buffer = Buffer::empty(Rect::new(0, 0, 5, size));
	let mut state = ScrollbarState::new(content_length).position(position);
	Scrollbar::new(ScrollbarOrientation::VerticalRight)
		.begin_symbol(Some("<"))
		.end_symbol(Some(">"))
		.track_symbol(Some("-"))
		.thumb_symbol("#")
		.render(buffer.area, &mut buffer, &mut state);
	let bar = expected.chars().map(|c| format!("    {c}"));
	assert_eq!(buffer, Buffer::with_lines(bar));
}

#[rstest]
#[case::position_0("##--------", 0, 10)]
#[case::position_1("-##-------", 1, 10)]
#[case::position_2("--##------", 2, 10)]
#[case::position_3("---##-----", 3, 10)]
#[case::position_4("----#-----", 4, 10)]
#[case::position_5("-----#----", 5, 10)]
#[case::position_6("-----##---", 6, 10)]
#[case::position_7("------##--", 7, 10)]
#[case::position_8("-------##-", 8, 10)]
#[case::position_9("--------##", 9, 10)]
#[case::position_one_out_of_bounds("--------##", 10, 10)]
fn custom_viewport_length(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
	scrollbar_no_arrows: Scrollbar,
) {
	let size = expected.width() as u16;
	let mut buffer = Buffer::empty(Rect::new(0, 0, size, 1));
	let mut state = ScrollbarState::new(content_length)
		.position(position)
		.viewport_content_length(2);
	scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines([expected]));
}

/// Fixes  which was a bug that would not
/// render a thumb when the viewport was very small in comparison to the content length.
#[rstest]
#[case::position_0("#----", 0, 100)]
#[case::position_10("#----", 10, 100)]
#[case::position_20("-#---", 20, 100)]
#[case::position_30("-#---", 30, 100)]
#[case::position_40("--#--", 40, 100)]
#[case::position_50("--#--", 50, 100)]
#[case::position_60("---#-", 60, 100)]
#[case::position_70("---#-", 70, 100)]
#[case::position_80("----#", 80, 100)]
#[case::position_90("----#", 90, 100)]
#[case::position_one_out_of_bounds("----#", 100, 100)]
fn thumb_visible_on_very_small_track(
	#[case] expected: &str,
	#[case] position: usize,
	#[case] content_length: usize,
	scrollbar_no_arrows: Scrollbar,
) {
	let size = expected.width() as u16;
	let mut buffer = Buffer::empty(Rect::new(0, 0, size, 1));
	let mut state = ScrollbarState::new(content_length)
		.position(position)
		.viewport_content_length(2);
	scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines([expected]));
}

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
