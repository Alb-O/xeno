use alloc::vec;

use rstest::rstest;

use super::*;
use crate::style::{Color, Modifier, Stylize};
use crate::{enum_display_from_str_tests, render_test};

#[test]
fn create_with_all_borders() {
	let block = Block::bordered();
	assert_eq!(block.borders, Borders::all());
}

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

#[test]
const fn border_type_can_be_const() {
	const _PLAIN: border::Set = BorderType::border_symbols(BorderType::Plain);
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

/// Renders a series of blocks with all the possible border types and merges them according to
/// the specified strategy. The resulting buffer is compared against the expected output for
/// each merge strategy.
///
/// At some point, it might be convenient to replace the manual `include_str!` calls with
/// [insta](https://crates.io/crates/insta)
/*
#[rstest]
#[case::replace(MergeStrategy::Replace, include_str!("../tests/block/merge_replace.txt"))]
#[case::exact(MergeStrategy::Exact, include_str!("../tests/block/merge_exact.txt"))]
#[case::fuzzy(MergeStrategy::Fuzzy, include_str!("../tests/block/merge_fuzzy.txt"))]
fn render_merged_borders(#[case] strategy: MergeStrategy, #[case] expected: &'static str) {
	let border_types = [
		BorderType::Plain,
		BorderType::Rounded,
		BorderType::Thick,
		BorderType::Double,
		BorderType::LightDoubleDashed,
		BorderType::HeavyDoubleDashed,
		BorderType::LightTripleDashed,
		BorderType::HeavyTripleDashed,
		BorderType::LightQuadrupleDashed,
		BorderType::HeavyQuadrupleDashed,
	];
	let rects = [
		// touching at corners
		(Rect::new(0, 0, 5, 5), Rect::new(4, 4, 5, 5)),
		// overlapping
		(Rect::new(10, 0, 5, 5), Rect::new(12, 2, 5, 5)),
		// touching vertical edges
		(Rect::new(18, 0, 5, 5), Rect::new(22, 0, 5, 5)),
		// touching horizontal edges
		(Rect::new(28, 0, 5, 5), Rect::new(28, 4, 5, 5)),
	];

	let mut buffer = Buffer::empty(Rect::new(0, 0, 43, 1000));

	let mut offset = Offset::ZERO;
	for (border_type_1, border_type_2) in iproduct!(border_types, border_types) {
		let title = format!("{border_type_1} + {border_type_2}");
		let title_area = Rect::new(0, 0, 43, 1) + offset;
		title.render(title_area, &mut buffer);
		offset.y += 1;
		for (rect_1, rect_2) in rects {
			Block::bordered()
				.border_type(border_type_1)
				.merge_borders(strategy)
				.render(rect_1 + offset, &mut buffer);
			Block::bordered()
				.border_type(border_type_2)
				.merge_borders(strategy)
				.render(rect_2 + offset, &mut buffer);
		}
		offset.y += 9;
	}
	pretty_assertions::assert_eq!(Buffer::with_lines(expected.lines()), buffer);
}
*/

#[rstest]
#[case::replace(MergeStrategy::Replace, Buffer::with_lines([
            "┏block top━━┓",
            "┃           ┃",
            "┗━━━━━━━━━━━┛",
            "│           │",
            "└───────────┘",
        ])
    )]
#[case::replace(MergeStrategy::Exact, Buffer::with_lines([
            "┏block top━━┓",
            "┃           ┃",
            "┡block btm━━┩",
            "│           │",
            "└───────────┘",
        ])
    )]
#[case::replace(MergeStrategy::Fuzzy, Buffer::with_lines([
            "┏block top━━┓",
            "┃           ┃",
            "┡block btm━━┩",
            "│           │",
            "└───────────┘",
        ])
    )]
fn merged_titles_bottom_first(#[case] strategy: MergeStrategy, #[case] expected: Buffer) {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 13, 5));
	Block::bordered()
		.border_type(BorderType::Plain)
		.title("block btm")
		.render(Rect::new(0, 2, 13, 3), &mut buffer);
	Block::bordered()
		.title("block top")
		.border_type(BorderType::Thick)
		.merge_borders(strategy)
		.render(Rect::new(0, 0, 13, 3), &mut buffer);
	assert_eq!(buffer, expected);
}

#[rstest]
#[case::replace(MergeStrategy::Replace, Buffer::with_lines([
            "┏block top━━┓",
            "┃           ┃",
            "┌block btm──┐",
            "│           │",
            "└───────────┘",
        ])
    )]
#[case::replace(MergeStrategy::Exact, Buffer::with_lines([
            "┏block top━━┓",
            "┃           ┃",
            "┞block btm──┦",
            "│           │",
            "└───────────┘",
        ])
    )]
#[case::replace(MergeStrategy::Fuzzy, Buffer::with_lines([
            "┏block top━━┓",
            "┃           ┃",
            "┞block btm──┦",
            "│           │",
            "└───────────┘",
        ])
    )]
fn merged_titles_top_first(#[case] strategy: MergeStrategy, #[case] expected: Buffer) {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 13, 5));
	Block::bordered()
		.title("block top")
		.border_type(BorderType::Thick)
		.render(Rect::new(0, 0, 13, 3), &mut buffer);
	Block::bordered()
		.border_type(BorderType::Plain)
		.title("block btm")
		.merge_borders(strategy)
		.render(Rect::new(0, 2, 13, 3), &mut buffer);
	assert_eq!(buffer, expected);
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
