use xeno_language::highlight::Highlight;
use xeno_tui::style::Color;

use super::*;

fn s(start: u32, end: u32, color: Color) -> (HighlightSpan, Style) {
	(
		HighlightSpan {
			start,
			end,
			highlight: Highlight::new(0),
		},
		Style::default().fg(color),
	)
}

#[test]
fn test_simple_overlap() {
	let spans = vec![s(0, 10, Color::Red), s(5, 15, Color::Blue)];
	let index = HighlightIndex::new(spans);
	// Expect [0,5] Red, [5,15] Blue (last wins)
	assert_eq!(index.spans.len(), 2);
	assert_eq!(index.spans[0].0, 0..5);
	assert_eq!(index.spans[0].1.fg, Some(Color::Red));
	assert_eq!(index.spans[1].0, 5..15);
	assert_eq!(index.spans[1].1.fg, Some(Color::Blue));
}

#[test]
fn test_nested_overlap() {
	let spans = vec![s(0, 10, Color::Red), s(2, 8, Color::Blue), s(4, 6, Color::Green)];
	let index = HighlightIndex::new(spans);
	// Expect Red, Blue, Green, Blue, Red
	assert_eq!(index.spans.len(), 5);
	assert_eq!(index.spans[0].0, 0..2);
	assert_eq!(index.spans[0].1.fg, Some(Color::Red));
	assert_eq!(index.spans[1].0, 2..4);
	assert_eq!(index.spans[1].1.fg, Some(Color::Blue));
	assert_eq!(index.spans[2].0, 4..6);
	assert_eq!(index.spans[2].1.fg, Some(Color::Green));
	assert_eq!(index.spans[3].0, 6..8);
	assert_eq!(index.spans[3].1.fg, Some(Color::Blue));
	assert_eq!(index.spans[4].0, 8..10);
	assert_eq!(index.spans[4].1.fg, Some(Color::Red));
}

#[test]
fn test_same_start_last_wins() {
	let spans = vec![s(0, 10, Color::Red), s(0, 5, Color::Blue)];
	let index = HighlightIndex::new(spans);
	// Since Blue comes later, it wins over Red for 0..5
	assert_eq!(index.spans.len(), 2);
	assert_eq!(index.spans[0].0, 0..5);
	assert_eq!(index.spans[0].1.fg, Some(Color::Blue));
	assert_eq!(index.spans[1].0, 5..10);
	assert_eq!(index.spans[1].1.fg, Some(Color::Red));
}

#[test]
fn test_same_end_priority() {
	let spans = vec![s(0, 10, Color::Red), s(5, 10, Color::Blue)];
	let index = HighlightIndex::new(spans);
	// Expect [0,5] Red, [5,10] Blue
	assert_eq!(index.spans.len(), 2);
	assert_eq!(index.spans[0].0, 0..5);
	assert_eq!(index.spans[0].1.fg, Some(Color::Red));
	assert_eq!(index.spans[1].0, 5..10);
	assert_eq!(index.spans[1].1.fg, Some(Color::Blue));
}

#[test]
fn test_end_start_adjacency() {
	let spans = vec![s(0, 5, Color::Red), s(5, 10, Color::Blue)];
	let index = HighlightIndex::new(spans);
	assert_eq!(index.spans.len(), 2);
	assert_eq!(index.spans[0].0, 0..5);
	assert_eq!(index.spans[1].0, 5..10);
}

#[test]
fn test_adjacent_merging() {
	let spans = vec![s(0, 5, Color::Red), s(5, 10, Color::Red)];
	let index = HighlightIndex::new(spans);
	assert_eq!(index.spans.len(), 1);
	assert_eq!(index.spans[0].0, 0..10);
}
