//! Tests for title rendering, alignment, and positioning.

use super::*;

#[test]
fn title_top_bottom() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 11, 3));
	Block::bordered()
		.border_type(BorderType::Plain)
		.title_top(Line::raw("A").left_aligned())
		.title_top(Line::raw("B").centered())
		.title_top(Line::raw("C").right_aligned())
		.title_bottom(Line::raw("D").left_aligned())
		.title_bottom(Line::raw("E").centered())
		.title_bottom(Line::raw("F").right_aligned())
		.render(buffer.area, &mut buffer);
	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"┌A───B───C┐",
		"│         │",
		"└D───E───F┘",
	]);
	assert_eq!(buffer, expected);
}

#[test]
fn title_alignment() {
	let tests = vec![
		(Alignment::Left, "test    "),
		(Alignment::Center, "  test  "),
		(Alignment::Right, "    test"),
	];
	for (alignment, expected) in tests {
		let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 1));
		Block::new()
			.title_alignment(alignment)
			.title("test")
			.render(buffer.area, &mut buffer);
		assert_eq!(buffer, Buffer::with_lines([expected]));
	}
}

#[test]
fn title_alignment_overrides_block_title_alignment() {
	let tests = vec![
		(Alignment::Right, Alignment::Left, "test    "),
		(Alignment::Left, Alignment::Center, "  test  "),
		(Alignment::Center, Alignment::Right, "    test"),
	];
	for (block_title_alignment, alignment, expected) in tests {
		let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 1));
		Block::new()
			.title_alignment(block_title_alignment)
			.title(Line::from("test").alignment(alignment))
			.render(buffer.area, &mut buffer);
		assert_eq!(buffer, Buffer::with_lines([expected]));
	}
}

/// This is a regression test for bug
#[test]
fn render_right_aligned_empty_title() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 15, 3));
	Block::new()
		.title_alignment(Alignment::Right)
		.title("")
		.render(buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(["               "; 3]));
}

#[test]
fn title_position() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 2));
	Block::new()
		.title_position(TitlePosition::Bottom)
		.title("test")
		.render(buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(["    ", "test"]));
}

#[test]
fn left_titles() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
	Block::new()
		.title("L12")
		.title("L34")
		.render(buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(["L12 L34   "]));
}

#[test]
fn left_titles_truncated() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
	Block::new()
		.title("L12345")
		.title("L67890")
		.render(buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(["L12345 L67"]));
}

#[test]
fn center_titles() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
	Block::new()
		.title(Line::from("C12").centered())
		.title(Line::from("C34").centered())
		.render(buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines([" C12 C34  "]));
}

#[test]
fn center_titles_truncated() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
	Block::new()
		.title(Line::from("C12345").centered())
		.title(Line::from("C67890").centered())
		.render(buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(["12345 C678"]));
}

#[test]
fn right_titles() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
	Block::new()
		.title(Line::from("R12").right_aligned())
		.title(Line::from("R34").right_aligned())
		.render(buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(["   R12 R34"]));
}

#[test]
fn right_titles_truncated() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
	Block::new()
		.title(Line::from("R12345").right_aligned())
		.title(Line::from("R67890").right_aligned())
		.render(buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(["345 R67890"]));
}

#[test]
fn center_title_truncates_left_title() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
	Block::new()
		.title("L1234")
		.title(Line::from("C5678").centered())
		.render(buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(["L1C5678   "]));
}

#[test]
fn right_title_truncates_left_title() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
	Block::new()
		.title("L12345")
		.title(Line::from("R67890").right_aligned())
		.render(buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(["L123R67890"]));
}

#[test]
fn right_title_truncates_center_title() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
	Block::new()
		.title(Line::from("C12345").centered())
		.title(Line::from("R67890").right_aligned())
		.render(buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(["  C1R67890"]));
}
