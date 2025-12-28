//! Tests for block styling - style, border_style, title_style.

use super::*;

/// Ensure Style from/into works the way a user would use it.
#[test]
fn style_into_works_from_user_view() {
	// nominal style
	let block = Block::new().style(Style::new().red());
	assert_eq!(block.style, Style::new().red());

	// auto-convert from Color
	let block = Block::new().style(Color::Red);
	assert_eq!(block.style, Style::new().red());

	// auto-convert from (Color, Color)
	let block = Block::new().style((Color::Red, Color::Blue));
	assert_eq!(block.style, Style::new().red().on_blue());

	// auto-convert from Modifier
	let block = Block::new().style(Modifier::BOLD | Modifier::ITALIC);
	assert_eq!(block.style, Style::new().bold().italic());

	// auto-convert from (Modifier, Modifier)
	let block = Block::new().style((Modifier::BOLD | Modifier::ITALIC, Modifier::DIM));
	assert_eq!(block.style, Style::new().bold().italic().not_dim());

	// auto-convert from (Color, Modifier)
	let block = Block::new().style((Color::Red, Modifier::BOLD));
	assert_eq!(block.style, Style::new().red().bold());

	// auto-convert from (Color, Color, Modifier)
	let block = Block::new().style((Color::Red, Color::Blue, Modifier::BOLD));
	assert_eq!(block.style, Style::new().red().on_blue().bold());

	// auto-convert from (Color, Color, Modifier, Modifier)
	let block = Block::new().style((
		Color::Red,
		Color::Blue,
		Modifier::BOLD | Modifier::ITALIC,
		Modifier::DIM,
	));
	assert_eq!(
		block.style,
		Style::new().red().on_blue().bold().italic().not_dim()
	);
}

#[test]
fn can_be_stylized() {
	let block = Block::new().black().on_white().bold().not_dim();
	assert_eq!(
		block.style,
		Style::default()
			.fg(Color::Black)
			.bg(Color::White)
			.add_modifier(Modifier::BOLD)
			.remove_modifier(Modifier::DIM)
	);
}

#[test]
fn title_content_style() {
	for alignment in [Alignment::Left, Alignment::Center, Alignment::Right] {
		let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 1));
		Block::new()
			.title_alignment(alignment)
			.title("test".yellow())
			.render(buffer.area, &mut buffer);
		assert_eq!(buffer, Buffer::with_lines(["test".yellow()]));
	}
}

#[test]
fn block_title_style() {
	for alignment in [Alignment::Left, Alignment::Center, Alignment::Right] {
		let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 1));
		Block::new()
			.title_alignment(alignment)
			.title_style(Style::new().yellow())
			.title("test")
			.render(buffer.area, &mut buffer);
		assert_eq!(buffer, Buffer::with_lines(["test".yellow()]));
	}
}

#[test]
fn title_style_overrides_block_title_style() {
	for alignment in [Alignment::Left, Alignment::Center, Alignment::Right] {
		let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 1));
		Block::new()
			.title_alignment(alignment)
			.title_style(Style::new().green().on_red())
			.title("test".yellow())
			.render(buffer.area, &mut buffer);
		assert_eq!(buffer, Buffer::with_lines(["test".yellow().on_red()]));
	}
}

#[test]
fn title_border_style() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
	Block::bordered()
		.border_type(BorderType::Plain)
		.title("test")
		.border_style(Style::new().yellow())
		.render(buffer.area, &mut buffer);
	#[rustfmt::skip]
	let mut expected = Buffer::with_lines([
		"┌test────┐",
		"│        │",
		"└────────┘",
	]);
	expected.set_style(Rect::new(0, 0, 10, 3), Style::new().yellow());
	expected.set_style(Rect::new(1, 1, 8, 1), Style::reset());
	assert_eq!(buffer, expected);
}
