//! Tests for merge strategies.

use super::*;

#[test]
fn replace_merge_strategy() {
	let strategy = MergeStrategy::Replace;
	let symbols = [
		"─", "━", "│", "┃", "┄", "┅", "┆", "┇", "┈", "┉", "┊", "┋", "┌", "┍", "┎", "┏", "┐",
		"┑", "┒", "┓", "└", "┕", "┖", "┗", "┘", "┙", "┚", "┛", "├", "┝", "┞", "┟", "┠", "┡",
		"┢", "┣", "┤", "┥", "┦", "┧", "┨", "┩", "┪", "┫", "┬", "┭", "┮", "┯", "┰", "┱", "┲",
		"┳", "┴", "┵", "┶", "┷", "┸", "┹", "┺", "┻", "┼", "┽", "┾", "┿", "╀", "╁", "╂", "╃",
		"╄", "╅", "╆", "╇", "╈", "╉", "╊", "╋", "╌", "╍", "╎", "╏", "═", "║", "╒", "╓", "╔",
		"╕", "╖", "╗", "╘", "╙", "╚", "╛", "╜", "╝", "╞", "╟", "╠", "╡", "╢", "╣", "╤", "╥",
		"╦", "╧", "╨", "╩", "╪", "╫", "╬", "╭", "╮", "╯", "╰", "╴", "╵", "╶", "╷", "╸", "╹",
		"╺", "╻", "╼", "╽", "╾", "╿", " ", "a", "b",
	];

	for a in symbols {
		for b in symbols {
			assert_eq!(strategy.merge(a, b), b);
		}
	}
}

#[test]
fn exact_merge_strategy() {
	let strategy = MergeStrategy::Exact;
	assert_eq!(strategy.merge("┆", "─"), "─");
	assert_eq!(strategy.merge("┏", "┆"), "┆");
	assert_eq!(strategy.merge("╎", "┉"), "┉");
	assert_eq!(strategy.merge("╎", "┉"), "┉");
	assert_eq!(strategy.merge("┋", "┋"), "┋");
	assert_eq!(strategy.merge("╷", "╶"), "┌");
	assert_eq!(strategy.merge("╭", "┌"), "┌");
	assert_eq!(strategy.merge("│", "┕"), "┝");
	assert_eq!(strategy.merge("┏", "│"), "┝");
	assert_eq!(strategy.merge("│", "┏"), "┢");
	assert_eq!(strategy.merge("╽", "┕"), "┢");
	assert_eq!(strategy.merge("│", "─"), "┼");
	assert_eq!(strategy.merge("┘", "┌"), "┼");
	assert_eq!(strategy.merge("┵", "┝"), "┿");
	assert_eq!(strategy.merge("│", "━"), "┿");
	assert_eq!(strategy.merge("┵", "╞"), "╞");
	assert_eq!(strategy.merge(" ", "╠"), " ");
	assert_eq!(strategy.merge("╠", " "), " ");
	assert_eq!(strategy.merge("╎", "╧"), "╧");
	assert_eq!(strategy.merge("╛", "╒"), "╪");
	assert_eq!(strategy.merge("│", "═"), "╪");
	assert_eq!(strategy.merge("╤", "╧"), "╪");
	assert_eq!(strategy.merge("╡", "╞"), "╪");
	assert_eq!(strategy.merge("┌", "╭"), "╭");
	assert_eq!(strategy.merge("┘", "╭"), "╭");
	assert_eq!(strategy.merge("┌", "a"), "a");
	assert_eq!(strategy.merge("a", "╭"), "a");
	assert_eq!(strategy.merge("a", "b"), "b");
}

#[test]
fn fuzzy_merge_strategy() {
	let strategy = MergeStrategy::Fuzzy;
	assert_eq!(strategy.merge("┄", "╴"), "─");
	assert_eq!(strategy.merge("│", "┆"), "┆");
	assert_eq!(strategy.merge(" ", "┉"), " ");
	assert_eq!(strategy.merge("┋", "┋"), "┋");
	assert_eq!(strategy.merge("╷", "╶"), "┌");
	assert_eq!(strategy.merge("╭", "┌"), "┌");
	assert_eq!(strategy.merge("│", "┕"), "┝");
	assert_eq!(strategy.merge("┏", "│"), "┝");
	assert_eq!(strategy.merge("┏", "┆"), "┝");
	assert_eq!(strategy.merge("│", "┏"), "┢");
	assert_eq!(strategy.merge("╽", "┕"), "┢");
	assert_eq!(strategy.merge("│", "─"), "┼");
	assert_eq!(strategy.merge("┆", "─"), "┼");
	assert_eq!(strategy.merge("┘", "┌"), "┼");
	assert_eq!(strategy.merge("┘", "╭"), "┼");
	assert_eq!(strategy.merge("╎", "┉"), "┿");
	assert_eq!(strategy.merge(" ", "╠"), " ");
	assert_eq!(strategy.merge("╠", " "), " ");
	assert_eq!(strategy.merge("┵", "╞"), "╪");
	assert_eq!(strategy.merge("╛", "╒"), "╪");
	assert_eq!(strategy.merge("│", "═"), "╪");
	assert_eq!(strategy.merge("╤", "╧"), "╪");
	assert_eq!(strategy.merge("╡", "╞"), "╪");
	assert_eq!(strategy.merge("╎", "╧"), "╪");
	assert_eq!(strategy.merge("┌", "╭"), "╭");
	assert_eq!(strategy.merge("┌", "a"), "a");
	assert_eq!(strategy.merge("a", "╭"), "a");
	assert_eq!(strategy.merge("a", "b"), "b");
}
