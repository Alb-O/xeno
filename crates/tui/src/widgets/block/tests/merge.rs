//! Tests for border merge strategies.

use rstest::rstest;

use super::*;

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
