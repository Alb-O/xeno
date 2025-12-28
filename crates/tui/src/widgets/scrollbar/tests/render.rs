//! Scrollbar rendering tests

use rstest::rstest;
use unicode_width::UnicodeWidthStr;

use super::*;

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
