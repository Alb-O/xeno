//! Basic constraint tests: Length, Max, Min, and constraint interactions.

use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;

use itertools::Itertools;
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::letters;
use crate::layout::Constraint::{self, *};
use crate::layout::{Direction, Flex, Layout, Rect};

#[rstest]
// flex, width, lengths, expected
#[case(Flex::Legacy, 1, &[Length(0)], "a")] // zero
#[case(Flex::Legacy, 1, &[Length(1)], "a")] // exact
#[case(Flex::Legacy, 1, &[Length(2)], "a")] // overflow
#[case(Flex::Legacy, 2, &[Length(0)], "aa")] // zero
#[case(Flex::Legacy, 2, &[Length(1)], "aa")] // underflow
#[case(Flex::Legacy, 2, &[Length(2)], "aa")] // exact
#[case(Flex::Legacy, 2, &[Length(3)], "aa")] // overflow
#[case(Flex::Legacy, 1, &[Length(0), Length(0)], "b")] // zero, zero
#[case(Flex::Legacy, 1, &[Length(0), Length(1)], "b")] // zero, exact
#[case(Flex::Legacy, 1, &[Length(0), Length(2)], "b")] // zero, overflow
#[case(Flex::Legacy, 1, &[Length(1), Length(0)], "a")] // exact, zero
#[case(Flex::Legacy, 1, &[Length(1), Length(1)], "a")] // exact, exact
#[case(Flex::Legacy, 1, &[Length(1), Length(2)], "a")] // exact, overflow
#[case(Flex::Legacy, 1, &[Length(2), Length(0)], "a")] // overflow, zero
#[case(Flex::Legacy, 1, &[Length(2), Length(1)], "a")] // overflow, exact
#[case(Flex::Legacy, 1, &[Length(2), Length(2)], "a")] // overflow, overflow
#[case(Flex::Legacy, 2, &[Length(0), Length(0)], "bb")] // zero, zero
#[case(Flex::Legacy, 2, &[Length(0), Length(1)], "bb")] // zero, underflow
#[case(Flex::Legacy, 2, &[Length(0), Length(2)], "bb")] // zero, exact
#[case(Flex::Legacy, 2, &[Length(0), Length(3)], "bb")] // zero, overflow
#[case(Flex::Legacy, 2, &[Length(1), Length(0)], "ab")] // underflow, zero
#[case(Flex::Legacy, 2, &[Length(1), Length(1)], "ab")] // underflow, underflow
#[case(Flex::Legacy, 2, &[Length(1), Length(2)], "ab")] // underflow, exact
#[case(Flex::Legacy, 2, &[Length(1), Length(3)], "ab")] // underflow, overflow
#[case(Flex::Legacy, 2, &[Length(2), Length(0)], "aa")] // exact, zero
#[case(Flex::Legacy, 2, &[Length(2), Length(1)], "aa")] // exact, underflow
#[case(Flex::Legacy, 2, &[Length(2), Length(2)], "aa")] // exact, exact
#[case(Flex::Legacy, 2, &[Length(2), Length(3)], "aa")] // exact, overflow
#[case(Flex::Legacy, 2, &[Length(3), Length(0)], "aa")] // overflow, zero
#[case(Flex::Legacy, 2, &[Length(3), Length(1)], "aa")] // overflow, underflow
#[case(Flex::Legacy, 2, &[Length(3), Length(2)], "aa")] // overflow, exact
#[case(Flex::Legacy, 2, &[Length(3), Length(3)], "aa")] // overflow, overflow
#[case(Flex::Legacy, 3, &[Length(2), Length(2)], "aab")] // with stretchlast
fn length(
	#[case] flex: Flex,
	#[case] width: u16,
	#[case] constraints: &[Constraint],
	#[case] expected: &str,
) {
	letters(flex, constraints, width, expected);
}

#[rstest]
#[case(Flex::Legacy, 1, &[Max(0)], "a")] // zero
#[case(Flex::Legacy, 1, &[Max(1)], "a")] // exact
#[case(Flex::Legacy, 1, &[Max(2)], "a")] // overflow
#[case(Flex::Legacy, 2, &[Max(0)], "aa")] // zero
#[case(Flex::Legacy, 2, &[Max(1)], "aa")] // underflow
#[case(Flex::Legacy, 2, &[Max(2)], "aa")] // exact
#[case(Flex::Legacy, 2, &[Max(3)], "aa")] // overflow
#[case(Flex::Legacy, 1, &[Max(0), Max(0)], "b")] // zero, zero
#[case(Flex::Legacy, 1, &[Max(0), Max(1)], "b")] // zero, exact
#[case(Flex::Legacy, 1, &[Max(0), Max(2)], "b")] // zero, overflow
#[case(Flex::Legacy, 1, &[Max(1), Max(0)], "a")] // exact, zero
#[case(Flex::Legacy, 1, &[Max(1), Max(1)], "a")] // exact, exact
#[case(Flex::Legacy, 1, &[Max(1), Max(2)], "a")] // exact, overflow
#[case(Flex::Legacy, 1, &[Max(2), Max(0)], "a")] // overflow, zero
#[case(Flex::Legacy, 1, &[Max(2), Max(1)], "a")] // overflow, exact
#[case(Flex::Legacy, 1, &[Max(2), Max(2)], "a")] // overflow, overflow
#[case(Flex::Legacy, 2, &[Max(0), Max(0)], "bb")] // zero, zero
#[case(Flex::Legacy, 2, &[Max(0), Max(1)], "bb")] // zero, underflow
#[case(Flex::Legacy, 2, &[Max(0), Max(2)], "bb")] // zero, exact
#[case(Flex::Legacy, 2, &[Max(0), Max(3)], "bb")] // zero, overflow
#[case(Flex::Legacy, 2, &[Max(1), Max(0)], "ab")] // underflow, zero
#[case(Flex::Legacy, 2, &[Max(1), Max(1)], "ab")] // underflow, underflow
#[case(Flex::Legacy, 2, &[Max(1), Max(2)], "ab")] // underflow, exact
#[case(Flex::Legacy, 2, &[Max(1), Max(3)], "ab")] // underflow, overflow
#[case(Flex::Legacy, 2, &[Max(2), Max(0)], "aa")] // exact, zero
#[case(Flex::Legacy, 2, &[Max(2), Max(1)], "aa")] // exact, underflow
#[case(Flex::Legacy, 2, &[Max(2), Max(2)], "aa")] // exact, exact
#[case(Flex::Legacy, 2, &[Max(2), Max(3)], "aa")] // exact, overflow
#[case(Flex::Legacy, 2, &[Max(3), Max(0)], "aa")] // overflow, zero
#[case(Flex::Legacy, 2, &[Max(3), Max(1)], "aa")] // overflow, underflow
#[case(Flex::Legacy, 2, &[Max(3), Max(2)], "aa")] // overflow, exact
#[case(Flex::Legacy, 2, &[Max(3), Max(3)], "aa")] // overflow, overflow
#[case(Flex::Legacy, 3, &[Max(2), Max(2)], "aab")]
fn max(
	#[case] flex: Flex,
	#[case] width: u16,
	#[case] constraints: &[Constraint],
	#[case] expected: &str,
) {
	letters(flex, constraints, width, expected);
}

#[rstest]
#[case(Flex::Legacy, 1, &[Min(0), Min(0)], "b")] // zero, zero
#[case(Flex::Legacy, 1, &[Min(0), Min(1)], "b")] // zero, exact
#[case(Flex::Legacy, 1, &[Min(0), Min(2)], "b")] // zero, overflow
#[case(Flex::Legacy, 1, &[Min(1), Min(0)], "a")] // exact, zero
#[case(Flex::Legacy, 1, &[Min(1), Min(1)], "a")] // exact, exact
#[case(Flex::Legacy, 1, &[Min(1), Min(2)], "a")] // exact, overflow
#[case(Flex::Legacy, 1, &[Min(2), Min(0)], "a")] // overflow, zero
#[case(Flex::Legacy, 1, &[Min(2), Min(1)], "a")] // overflow, exact
#[case(Flex::Legacy, 1, &[Min(2), Min(2)], "a")] // overflow, overflow
#[case(Flex::Legacy, 2, &[Min(0), Min(0)], "bb")] // zero, zero
#[case(Flex::Legacy, 2, &[Min(0), Min(1)], "bb")] // zero, underflow
#[case(Flex::Legacy, 2, &[Min(0), Min(2)], "bb")] // zero, exact
#[case(Flex::Legacy, 2, &[Min(0), Min(3)], "bb")] // zero, overflow
#[case(Flex::Legacy, 2, &[Min(1), Min(0)], "ab")] // underflow, zero
#[case(Flex::Legacy, 2, &[Min(1), Min(1)], "ab")] // underflow, underflow
#[case(Flex::Legacy, 2, &[Min(1), Min(2)], "ab")] // underflow, exact
#[case(Flex::Legacy, 2, &[Min(1), Min(3)], "ab")] // underflow, overflow
#[case(Flex::Legacy, 2, &[Min(2), Min(0)], "aa")] // exact, zero
#[case(Flex::Legacy, 2, &[Min(2), Min(1)], "aa")] // exact, underflow
#[case(Flex::Legacy, 2, &[Min(2), Min(2)], "aa")] // exact, exact
#[case(Flex::Legacy, 2, &[Min(2), Min(3)], "aa")] // exact, overflow
#[case(Flex::Legacy, 2, &[Min(3), Min(0)], "aa")] // overflow, zero
#[case(Flex::Legacy, 2, &[Min(3), Min(1)], "aa")] // overflow, underflow
#[case(Flex::Legacy, 2, &[Min(3), Min(2)], "aa")] // overflow, exact
#[case(Flex::Legacy, 2, &[Min(3), Min(3)], "aa")] // overflow, overflow
#[case(Flex::Legacy, 3, &[Min(2), Min(2)], "aab")]
fn min(
	#[case] flex: Flex,
	#[case] width: u16,
	#[case] constraints: &[Constraint],
	#[case] expected: &str,
) {
	letters(flex, constraints, width, expected);
}

fn edge_cases() {
	// stretches into last
	let layout = Layout::default()
		.constraints([
			Constraint::Percentage(50),
			Constraint::Percentage(50),
			Constraint::Min(0),
		])
		.split(Rect::new(0, 0, 1, 1));
	assert_eq!(
		layout[..],
		[
			Rect::new(0, 0, 1, 1),
			Rect::new(0, 1, 1, 0),
			Rect::new(0, 1, 1, 0)
		]
	);

	// stretches into last
	let layout = Layout::default()
		.constraints([
			Constraint::Max(1),
			Constraint::Percentage(99),
			Constraint::Min(0),
		])
		.split(Rect::new(0, 0, 1, 1));
	assert_eq!(
		layout[..],
		[
			Rect::new(0, 0, 1, 0),
			Rect::new(0, 0, 1, 1),
			Rect::new(0, 1, 1, 0)
		]
	);

	// minimal bug from
	// #issuecomment-1681850644
	// TODO: check if this bug is now resolved?
	let layout = Layout::default()
		.constraints([Min(1), Length(0), Min(1)])
		.direction(Direction::Horizontal)
		.split(Rect::new(0, 0, 1, 1));
	assert_eq!(
		layout[..],
		[
			Rect::new(0, 0, 1, 1),
			Rect::new(1, 0, 0, 1),
			Rect::new(1, 0, 0, 1),
		]
	);

	// This stretches the 2nd last length instead of the last min based on ranking
	let layout = Layout::default()
		.constraints([Length(3), Min(4), Length(1), Min(4)])
		.direction(Direction::Horizontal)
		.split(Rect::new(0, 0, 7, 1));
	assert_eq!(
		layout[..],
		[
			Rect::new(0, 0, 0, 1),
			Rect::new(0, 0, 4, 1),
			Rect::new(4, 0, 0, 1),
			Rect::new(4, 0, 3, 1),
		]
	);
}
#[rstest]
#[case::len_min1(vec![Length(25), Min(100)], vec![0..0,  0..100])]
#[case::len_min2(vec![Length(25), Min(0)], vec![0..25, 25..100])]
#[case::len_max1(vec![Length(25), Max(0)], vec![0..100, 100..100])]
#[case::len_max2(vec![Length(25), Max(100)], vec![0..25, 25..100])]
#[case::len_perc(vec![Length(25), Percentage(25)], vec![0..25, 25..100])]
#[case::perc_len(vec![Percentage(25), Length(25)], vec![0..75, 75..100])]
#[case::len_ratio(vec![Length(25), Ratio(1, 4)], vec![0..25, 25..100])]
#[case::ratio_len(vec![Ratio(1, 4), Length(25)], vec![0..75, 75..100])]
#[case::len_len(vec![Length(25), Length(25)], vec![0..25, 25..100])]
#[case::len1(vec![Length(25), Length(25), Length(25)], vec![0..25, 25..50, 50..100])]
#[case::len2(vec![Length(15), Length(35), Length(25)], vec![0..15, 15..50, 50..100])]
#[case::len3(vec![Length(25), Length(25), Length(25)], vec![0..25, 25..50, 50..100])]
fn constraint_length(#[case] constraints: Vec<Constraint>, #[case] expected: Vec<Range<u16>>) {
	let rect = Rect::new(0, 0, 100, 1);
	let ranges = Layout::horizontal(constraints)
		.flex(Flex::Legacy)
		.split(rect)
		.iter()
		.map(|r| r.left()..r.right())
		.collect_vec();
	assert_eq!(ranges, expected);
}

#[rstest]
#[case(7, vec![Length(4), Length(4)], vec![0..3, 4..7])]
#[case(4, vec![Length(4), Length(4)], vec![0..2, 3..4])]
fn table_length(
	#[case] width: u16,
	#[case] constraints: Vec<Constraint>,
	#[case] expected: Vec<Range<u16>>,
) {
	let rect = Rect::new(0, 0, width, 1);
	let ranges = Layout::horizontal(constraints)
		.spacing(1)
		.flex(Flex::Start)
		.split(rect)
		.iter()
		.map(|r| r.left()..r.right())
		.collect::<Vec<Range<u16>>>();
	assert_eq!(ranges, expected);
}

#[rstest]
#[case::min_len_max(vec![Min(25), Length(25), Max(25)], vec![0..50, 50..75, 75..100])]
#[case::max_len_min(vec![Max(25), Length(25), Min(25)], vec![0..25, 25..50, 50..100])]
#[case::len_len_len(vec![Length(33), Length(33), Length(33)], vec![0..33, 33..66, 66..100])]
#[case::len_len_len_25(vec![Length(25), Length(25), Length(25)], vec![0..25, 25..50, 50..100])]
#[case::perc_len_ratio(vec![Percentage(25), Length(25), Ratio(1, 4)], vec![0..25, 25..50, 50..100])]
#[case::len_ratio_perc(vec![Length(25), Ratio(1, 4), Percentage(25)], vec![0..25, 25..75, 75..100])]
#[case::ratio_len_perc(vec![Ratio(1, 4), Length(25), Percentage(25)], vec![0..50, 50..75, 75..100])]
#[case::ratio_perc_len(vec![Ratio(1, 4), Percentage(25), Length(25)], vec![0..50, 50..75, 75..100])]
#[case::len_len_min(vec![Length(100), Length(1), Min(20)], vec![0..80, 80..80, 80..100])]
#[case::min_len_len(vec![Min(20), Length(1), Length(100)], vec![0..20, 20..21, 21..100])]
#[case::fill_len_fill(vec![Fill(1), Length(10), Fill(1)], vec![0..45, 45..55, 55..100])]
#[case::fill_len_fill_2(vec![Fill(1), Length(10), Fill(2)], vec![0..30, 30..40, 40..100])]
#[case::fill_len_fill_4(vec![Fill(1), Length(10), Fill(4)], vec![0..18, 18..28, 28..100])]
#[case::fill_len_fill_5(vec![Fill(1), Length(10), Fill(5)], vec![0..15, 15..25, 25..100])]
#[case::len_len_len_25(vec![Length(25), Length(25), Length(25)], vec![0..25, 25..50, 50..100])]
#[case::unstable_test(vec![Length(25), Length(25), Length(25)], vec![0..25, 25..50, 50..100])]
fn length_is_higher_priority(
	#[case] constraints: Vec<Constraint>,
	#[case] expected: Vec<Range<u16>>,
) {
	let rect = Rect::new(0, 0, 100, 1);
	let ranges = Layout::horizontal(constraints)
		.flex(Flex::Legacy)
		.split(rect)
		.iter()
		.map(|r| r.left()..r.right())
		.collect_vec();
	assert_eq!(ranges, expected);
}

#[rstest]
#[case::min_len_max(vec![Min(25), Length(25), Max(25)], vec![50, 25, 25])]
#[case::max_len_min(vec![Max(25), Length(25), Min(25)], vec![25, 25, 50])]
#[case::len_len_len1(vec![Length(33), Length(33), Length(33)], vec![33, 33, 33])]
#[case::len_len_len2(vec![Length(25), Length(25), Length(25)], vec![25, 25, 25])]
#[case::perc_len_ratio(vec![Percentage(25), Length(25), Ratio(1, 4)], vec![25, 25, 25])]
#[case::len_ratio_perc(vec![Length(25), Ratio(1, 4), Percentage(25)], vec![25, 25, 25])]
#[case::ratio_len_perc(vec![Ratio(1, 4), Length(25), Percentage(25)], vec![25, 25, 25])]
#[case::ratio_perc_len(vec![Ratio(1, 4), Percentage(25), Length(25)], vec![25, 25, 25])]
#[case::len_len_min(vec![Length(100), Length(1), Min(20)], vec![79, 1, 20])]
#[case::min_len_len(vec![Min(20), Length(1), Length(100)], vec![20, 1, 79])]
#[case::fill_len_fill1(vec![Fill(1), Length(10), Fill(1)], vec![45, 10, 45])]
#[case::fill_len_fill2(vec![Fill(1), Length(10), Fill(2)], vec![30, 10, 60])]
#[case::fill_len_fill4(vec![Fill(1), Length(10), Fill(4)], vec![18, 10, 72])]
#[case::fill_len_fill5(vec![Fill(1), Length(10), Fill(5)], vec![15, 10, 75])]
#[case::len_len_len3(vec![Length(25), Length(25), Length(25)], vec![25, 25, 25])]
fn length_is_higher_priority_in_flex(
	#[case] constraints: Vec<Constraint>,
	#[case] expected: Vec<u16>,
) {
	let rect = Rect::new(0, 0, 100, 1);
	for flex in [
		Flex::Start,
		Flex::End,
		Flex::Center,
		Flex::SpaceAround,
		Flex::SpaceEvenly,
		Flex::SpaceBetween,
	] {
		let widths = Layout::horizontal(&constraints)
			.flex(flex)
			.split(rect)
			.iter()
			.map(|r| r.width)
			.collect_vec();
		assert_eq!(widths, expected);
	}
}
