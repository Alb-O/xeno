//! Tests for scrolling region functionality (feature-gated).

use rstest::rstest;

use super::*;

const A: &str = "aaaa";
const B: &str = "bbbb";
const C: &str = "cccc";
const D: &str = "dddd";
const E: &str = "eeee";
const S: &str = "    ";

#[rstest]
#[case([A, B, C, D, E], 0..5, 0, [],                    [A, B, C, D, E])]
#[case([A, B, C, D, E], 0..5, 2, [A, B],                [C, D, E, S, S])]
#[case([A, B, C, D, E], 0..5, 5, [A, B, C, D, E],       [S, S, S, S, S])]
#[case([A, B, C, D, E], 0..5, 7, [A, B, C, D, E, S, S], [S, S, S, S, S])]
#[case([A, B, C, D, E], 0..3, 0, [],                    [A, B, C, D, E])]
#[case([A, B, C, D, E], 0..3, 2, [A, B],                [C, S, S, D, E])]
#[case([A, B, C, D, E], 0..3, 3, [A, B, C],             [S, S, S, D, E])]
#[case([A, B, C, D, E], 0..3, 4, [A, B, C, S],          [S, S, S, D, E])]
#[case([A, B, C, D, E], 1..4, 0, [],                    [A, B, C, D, E])]
#[case([A, B, C, D, E], 1..4, 2, [],                    [A, D, S, S, E])]
#[case([A, B, C, D, E], 1..4, 3, [],                    [A, S, S, S, E])]
#[case([A, B, C, D, E], 1..4, 4, [],                    [A, S, S, S, E])]
#[case([A, B, C, D, E], 0..0, 0, [],                    [A, B, C, D, E])]
#[case([A, B, C, D, E], 0..0, 2, [S, S],                [A, B, C, D, E])]
#[case([A, B, C, D, E], 2..2, 0, [],                    [A, B, C, D, E])]
#[case([A, B, C, D, E], 2..2, 2, [],                    [A, B, C, D, E])]
fn scroll_region_up<const L: usize, const M: usize, const N: usize>(
	#[case] initial_screen: [&'static str; L],
	#[case] range: core::ops::Range<u16>,
	#[case] scroll_by: u16,
	#[case] expected_scrollback: [&'static str; M],
	#[case] expected_buffer: [&'static str; N],
) {
	let mut backend = TestBackend::with_lines(initial_screen);
	backend.scroll_region_up(range, scroll_by).unwrap();
	if expected_scrollback.is_empty() {
		backend.assert_scrollback_empty();
	} else {
		backend.assert_scrollback_lines(expected_scrollback);
	}
	backend.assert_buffer_lines(expected_buffer);
}

#[rstest]
#[case([A, B, C, D, E], 0..5, 0, [A, B, C, D, E])]
#[case([A, B, C, D, E], 0..5, 2, [S, S, A, B, C])]
#[case([A, B, C, D, E], 0..5, 5, [S, S, S, S, S])]
#[case([A, B, C, D, E], 0..5, 7, [S, S, S, S, S])]
#[case([A, B, C, D, E], 0..3, 0, [A, B, C, D, E])]
#[case([A, B, C, D, E], 0..3, 2, [S, S, A, D, E])]
#[case([A, B, C, D, E], 0..3, 3, [S, S, S, D, E])]
#[case([A, B, C, D, E], 0..3, 4, [S, S, S, D, E])]
#[case([A, B, C, D, E], 1..4, 0, [A, B, C, D, E])]
#[case([A, B, C, D, E], 1..4, 2, [A, S, S, B, E])]
#[case([A, B, C, D, E], 1..4, 3, [A, S, S, S, E])]
#[case([A, B, C, D, E], 1..4, 4, [A, S, S, S, E])]
#[case([A, B, C, D, E], 0..0, 0, [A, B, C, D, E])]
#[case([A, B, C, D, E], 0..0, 2, [A, B, C, D, E])]
#[case([A, B, C, D, E], 2..2, 0, [A, B, C, D, E])]
#[case([A, B, C, D, E], 2..2, 2, [A, B, C, D, E])]
fn scroll_region_down<const M: usize, const N: usize>(
	#[case] initial_screen: [&'static str; M],
	#[case] range: core::ops::Range<u16>,
	#[case] scroll_by: u16,
	#[case] expected_buffer: [&'static str; N],
) {
	let mut backend = TestBackend::with_lines(initial_screen);
	backend.scroll_region_down(range, scroll_by).unwrap();
	backend.assert_scrollback_empty();
	backend.assert_buffer_lines(expected_buffer);
}
