//! Spacer and split_with_spacers tests.

use alloc::vec;
use alloc::vec::Vec;

use pretty_assertions::assert_eq;
use rstest::rstest;

use crate::layout::Constraint::{self, *};
use crate::layout::{Flex, Layout, Rect};

#[rstest]
#[case::flex_length10(vec![(0, 10), (90, 10)], vec![Length(10), Length(10)], Flex::Center, 80)]
fn flex_spacing_lower_priority_than_user_spacing(
	#[case] expected: Vec<(u16, u16)>,
	#[case] constraints: Vec<Constraint>,
	#[case] flex: Flex,
	#[case] spacing: i16,
) {
	let rect = Rect::new(0, 0, 100, 1);
	let r = Layout::horizontal(constraints)
		.flex(flex)
		.spacing(spacing)
		.split(rect);
	let result = r
		.iter()
		.map(|r| (r.x, r.width))
		.collect::<Vec<(u16, u16)>>();
	assert_eq!(result, expected);
}

#[rstest]
#[case::spacers(vec![(0, 0), (10, 0), (100, 0)], vec![Length(10), Length(10)], Flex::Legacy)]
#[case::spacers(vec![(0, 0), (10, 80), (100, 0)], vec![Length(10), Length(10)], Flex::SpaceBetween)]
#[case::spacers(vec![(0, 27), (37, 26), (73, 27)], vec![Length(10), Length(10)], Flex::SpaceEvenly)]
#[case::spacers(vec![(0, 20), (30, 40), (80, 20)], vec![Length(10), Length(10)], Flex::SpaceAround)]
#[case::spacers(vec![(0, 0), (10, 0), (20, 80)], vec![Length(10), Length(10)], Flex::Start)]
#[case::spacers(vec![(0, 40), (50, 0), (60, 40)], vec![Length(10), Length(10)], Flex::Center)]
#[case::spacers(vec![(0, 80), (90, 0), (100, 0)], vec![Length(10), Length(10)], Flex::End)]
fn split_with_spacers_no_spacing(
	#[case] expected: Vec<(u16, u16)>,
	#[case] constraints: Vec<Constraint>,
	#[case] flex: Flex,
) {
	let rect = Rect::new(0, 0, 100, 1);
	let (_, s) = Layout::horizontal(&constraints)
		.flex(flex)
		.split_with_spacers(rect);
	assert_eq!(s.len(), constraints.len() + 1);
	let result = s
		.iter()
		.map(|r| (r.x, r.width))
		.collect::<Vec<(u16, u16)>>();
	assert_eq!(result, expected);
}

#[rstest]
#[case::spacers(vec![(0, 0), (10, 5), (100, 0)], vec![Length(10), Length(10)], Flex::Legacy, 5)]
#[case::spacers(vec![(0, 0), (10, 80), (100, 0)], vec![Length(10), Length(10)], Flex::SpaceBetween, 5)]
#[case::spacers(vec![(0, 27), (37, 26), (73, 27)], vec![Length(10), Length(10)], Flex::SpaceEvenly, 5)]
#[case::spacers(vec![(0, 20), (30, 40), (80, 20)], vec![Length(10), Length(10)], Flex::SpaceAround, 5)]
#[case::spacers(vec![(0, 0), (10, 5), (25, 75)], vec![Length(10), Length(10)], Flex::Start, 5)]
#[case::spacers(vec![(0, 38), (48, 5), (63, 37)], vec![Length(10), Length(10)], Flex::Center, 5)]
#[case::spacers(vec![(0, 75), (85, 5), (100, 0)], vec![Length(10), Length(10)], Flex::End, 5)]
fn split_with_spacers_and_spacing(
	#[case] expected: Vec<(u16, u16)>,
	#[case] constraints: Vec<Constraint>,
	#[case] flex: Flex,
	#[case] spacing: i16,
) {
	let rect = Rect::new(0, 0, 100, 1);
	let (_, s) = Layout::horizontal(&constraints)
		.flex(flex)
		.spacing(spacing)
		.split_with_spacers(rect);
	assert_eq!(s.len(), constraints.len() + 1);
	let result = s
		.iter()
		.map(|r| (r.x, r.width))
		.collect::<Vec<(u16, u16)>>();
	assert_eq!(expected, result);
}

#[rstest]
#[case::spacers_1(vec![(0, 0), (10, 0), (100, 0)], vec![Length(10), Length(10)], Flex::Legacy, -1)]
#[case::spacers_2(vec![(0, 0), (10, 80), (100, 0)], vec![Length(10), Length(10)], Flex::SpaceBetween, -1)]
#[case::spacers_3(vec![(0, 27), (37, 26), (73, 27)], vec![Length(10), Length(10)], Flex::SpaceEvenly, -1)]
#[case::spacers_3(vec![(0, 20), (30, 40), (80, 20)], vec![Length(10), Length(10)], Flex::SpaceAround, -1)]
#[case::spacers_4(vec![(0, 0), (10, 0), (19, 81)], vec![Length(10), Length(10)], Flex::Start, -1)]
#[case::spacers_5(vec![(0, 41), (51, 0), (60, 40)], vec![Length(10), Length(10)], Flex::Center, -1)]
#[case::spacers_6(vec![(0, 81), (91, 0), (100, 0)], vec![Length(10), Length(10)], Flex::End, -1)]
fn split_with_spacers_and_overlap(
	#[case] expected: Vec<(u16, u16)>,
	#[case] constraints: Vec<Constraint>,
	#[case] flex: Flex,
	#[case] spacing: i16,
) {
	let rect = Rect::new(0, 0, 100, 1);
	let (_, s) = Layout::horizontal(&constraints)
		.flex(flex)
		.spacing(spacing)
		.split_with_spacers(rect);
	assert_eq!(s.len(), constraints.len() + 1);
	let result = s
		.iter()
		.map(|r| (r.x, r.width))
		.collect::<Vec<(u16, u16)>>();
	assert_eq!(result, expected);
}

#[rstest]
#[case::spacers(vec![(0, 0), (0, 100), (100, 0)], vec![Length(10), Length(10)], Flex::Legacy, 200)]
#[case::spacers(vec![(0, 0), (0, 100), (100, 0)], vec![Length(10), Length(10)], Flex::SpaceBetween, 200)]
#[case::spacers(vec![(0, 33), (33, 34), (67, 33)], vec![Length(10), Length(10)], Flex::SpaceEvenly, 200)]
#[case::spacers(vec![(0, 25), (25, 50), (75, 25)], vec![Length(10), Length(10)], Flex::SpaceAround, 200)]
#[case::spacers(vec![(0, 0), (0, 100), (100, 0)], vec![Length(10), Length(10)], Flex::Start, 200)]
#[case::spacers(vec![(0, 0), (0, 100), (100, 0)], vec![Length(10), Length(10)], Flex::Center, 200)]
#[case::spacers(vec![(0, 0), (0, 100), (100, 0)], vec![Length(10), Length(10)], Flex::End, 200)]
fn split_with_spacers_and_too_much_spacing(
	#[case] expected: Vec<(u16, u16)>,
	#[case] constraints: Vec<Constraint>,
	#[case] flex: Flex,
	#[case] spacing: i16,
) {
	let rect = Rect::new(0, 0, 100, 1);
	let (_, s) = Layout::horizontal(&constraints)
		.flex(flex)
		.spacing(spacing)
		.split_with_spacers(rect);
	assert_eq!(s.len(), constraints.len() + 1);
	let result = s
		.iter()
		.map(|r| (r.x, r.width))
		.collect::<Vec<(u16, u16)>>();
	assert_eq!(result, expected);
}

#[rstest]
#[case::compare(vec![(0, 90), (90, 10)], vec![Min(10), Length(10)], Flex::Legacy)]
#[case::compare(vec![(0, 90), (90, 10)], vec![Min(10), Length(10)], Flex::Start)]
#[case::compare(vec![(0, 10), (10, 90)], vec![Min(10), Percentage(100)], Flex::Legacy)]
#[case::compare(vec![(0, 10), (10, 90)], vec![Min(10), Percentage(100)], Flex::Start)]
#[case::compare(vec![(0, 50), (50, 50)], vec![Percentage(50), Percentage(50)], Flex::Legacy)]
#[case::compare(vec![(0, 50), (50, 50)], vec![Percentage(50), Percentage(50)], Flex::Start)]
fn legacy_vs_default(
	#[case] expected: Vec<(u16, u16)>,
	#[case] constraints: Vec<Constraint>,
	#[case] flex: Flex,
) {
	let rect = Rect::new(0, 0, 100, 1);
	let r = Layout::horizontal(constraints).flex(flex).split(rect);
	let result = r
		.iter()
		.map(|r| (r.x, r.width))
		.collect::<Vec<(u16, u16)>>();
	assert_eq!(result, expected);
}
