//! Flex mode tests.

use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;

use itertools::Itertools;
use pretty_assertions::assert_eq;
use rstest::rstest;

use crate::layout::Constraint::{self, *};
use crate::layout::{Direction, Flex, Layout, Rect};

#[test]
fn vertical_split_by_height() {
	let target = Rect {
		x: 2,
		y: 2,
		width: 10,
		height: 10,
	};

	let chunks = Layout::default()
		.direction(Direction::Vertical)
		.constraints([
			Constraint::Percentage(10),
			Constraint::Max(5),
			Constraint::Min(1),
		])
		.split(target);

	assert_eq!(chunks.iter().map(|r| r.height).sum::<u16>(), target.height);
	chunks.windows(2).for_each(|w| assert!(w[0].y <= w[1].y));
}

#[rstest]
#[case::max_min(vec![Max(100), Min(0)], vec![0..100, 100..100])]
#[case::min_max(vec![Min(0), Max(100)], vec![0..0, 0..100])]
#[case::length_min(vec![Length(u16::MAX), Min(10)], vec![0..90, 90..100])]
#[case::min_length(vec![Min(10), Length(u16::MAX)], vec![0..10, 10..100])]
#[case::length_max(vec![Length(0), Max(10)], vec![0..90, 90..100])]
#[case::max_length(vec![Max(10), Length(0)], vec![0..10, 10..100])]
fn min_max(#[case] constraints: Vec<Constraint>, #[case] expected: Vec<Range<u16>>) {
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
#[case::length_legacy(vec![Length(50)], vec![0..100], Flex::Legacy)]
#[case::length_start(vec![Length(50)], vec![0..50], Flex::Start)]
#[case::length_end(vec![Length(50)], vec![50..100], Flex::End)]
#[case::length_center(vec![Length(50)], vec![25..75], Flex::Center)]
#[case::ratio_legacy(vec![Ratio(1, 2)], vec![0..100], Flex::Legacy)]
#[case::ratio_start(vec![Ratio(1, 2)], vec![0..50], Flex::Start)]
#[case::ratio_end(vec![Ratio(1, 2)], vec![50..100], Flex::End)]
#[case::ratio_center(vec![Ratio(1, 2)], vec![25..75], Flex::Center)]
#[case::percent_legacy(vec![Percentage(50)], vec![0..100], Flex::Legacy)]
#[case::percent_start(vec![Percentage(50)], vec![0..50], Flex::Start)]
#[case::percent_end(vec![Percentage(50)], vec![50..100], Flex::End)]
#[case::percent_center(vec![Percentage(50)], vec![25..75], Flex::Center)]
#[case::min_legacy(vec![Min(50)], vec![0..100], Flex::Legacy)]
#[case::min_start(vec![Min(50)], vec![0..100], Flex::Start)]
#[case::min_end(vec![Min(50)], vec![0..100], Flex::End)]
#[case::min_center(vec![Min(50)], vec![0..100], Flex::Center)]
#[case::max_legacy(vec![Max(50)], vec![0..100], Flex::Legacy)]
#[case::max_start(vec![Max(50)], vec![0..50], Flex::Start)]
#[case::max_end(vec![Max(50)], vec![50..100], Flex::End)]
#[case::max_center(vec![Max(50)], vec![25..75], Flex::Center)]
#[case::spacebetween_becomes_stretch1(vec![Min(1)], vec![0..100], Flex::SpaceBetween)]
#[case::spacebetween_becomes_stretch2(vec![Max(20)], vec![0..100], Flex::SpaceBetween)]
#[case::spacebetween_becomes_stretch3(vec![Length(20)], vec![0..100], Flex::SpaceBetween)]
#[case::length_legacy2(vec![Length(25), Length(25)], vec![0..25, 25..100], Flex::Legacy)]
#[case::length_start2(vec![Length(25), Length(25)], vec![0..25, 25..50], Flex::Start)]
#[case::length_center2(vec![Length(25), Length(25)], vec![25..50, 50..75], Flex::Center)]
#[case::length_end2(vec![Length(25), Length(25)], vec![50..75, 75..100], Flex::End)]
#[case::length_spacebetween(vec![Length(25), Length(25)], vec![0..25, 75..100], Flex::SpaceBetween)]
#[case::length_spaceevenly(vec![Length(25), Length(25)], vec![17..42, 58..83], Flex::SpaceEvenly)]
#[case::length_spacearound(vec![Length(25), Length(25)], vec![13..38, 63..88], Flex::SpaceAround)]
#[case::percentage_legacy(vec![Percentage(25), Percentage(25)], vec![0..25, 25..100], Flex::Legacy)]
#[case::percentage_start(vec![Percentage(25), Percentage(25)], vec![0..25, 25..50], Flex::Start)]
#[case::percentage_center(vec![Percentage(25), Percentage(25)], vec![25..50, 50..75], Flex::Center)]
#[case::percentage_end(vec![Percentage(25), Percentage(25)], vec![50..75, 75..100], Flex::End)]
#[case::percentage_spacebetween(vec![Percentage(25), Percentage(25)], vec![0..25, 75..100], Flex::SpaceBetween)]
#[case::percentage_spaceevenly(vec![Percentage(25), Percentage(25)], vec![17..42, 58..83], Flex::SpaceEvenly)]
#[case::percentage_spacearound(vec![Percentage(25), Percentage(25)], vec![13..38, 63..88], Flex::SpaceAround)]
#[case::min_legacy2(vec![Min(25), Min(25)], vec![0..25, 25..100], Flex::Legacy)]
#[case::min_start2(vec![Min(25), Min(25)], vec![0..50, 50..100], Flex::Start)]
#[case::min_center2(vec![Min(25), Min(25)], vec![0..50, 50..100], Flex::Center)]
#[case::min_end2(vec![Min(25), Min(25)], vec![0..50, 50..100], Flex::End)]
#[case::min_spacebetween(vec![Min(25), Min(25)], vec![0..50, 50..100], Flex::SpaceBetween)]
#[case::min_spaceevenly(vec![Min(25), Min(25)], vec![0..50, 50..100], Flex::SpaceEvenly)]
#[case::min_spacearound(vec![Min(25), Min(25)], vec![0..50, 50..100], Flex::SpaceAround)]
#[case::max_legacy2(vec![Max(25), Max(25)], vec![0..25, 25..100], Flex::Legacy)]
#[case::max_start2(vec![Max(25), Max(25)], vec![0..25, 25..50], Flex::Start)]
#[case::max_center2(vec![Max(25), Max(25)], vec![25..50, 50..75], Flex::Center)]
#[case::max_end2(vec![Max(25), Max(25)], vec![50..75, 75..100], Flex::End)]
#[case::max_spacebetween(vec![Max(25), Max(25)], vec![0..25, 75..100], Flex::SpaceBetween)]
#[case::max_spaceevenly(vec![Max(25), Max(25)], vec![17..42, 58..83], Flex::SpaceEvenly)]
#[case::max_spacearound(vec![Max(25), Max(25)], vec![13..38, 63..88], Flex::SpaceAround)]
#[case::length_spaced_around(vec![Length(25), Length(25), Length(25)], vec![0..25, 38..63, 75..100], Flex::SpaceBetween)]
#[case::one_segment_legacy(vec![Length(50)], vec![0..100], Flex::Legacy)]
#[case::one_segment_start(vec![Length(50)], vec![0..50], Flex::Start)]
#[case::one_segment_end(vec![Length(50)], vec![50..100], Flex::End)]
#[case::one_segment_center(vec![Length(50)], vec![25..75], Flex::Center)]
#[case::one_segment_spacebetween(vec![Length(50)], vec![0..100], Flex::SpaceBetween)]
#[case::one_segment_spaceevenly(vec![Length(50)], vec![25..75], Flex::SpaceEvenly)]
#[case::one_segment_spacearound(vec![Length(50)], vec![25..75], Flex::SpaceAround)]
fn flex_constraint(
	#[case] constraints: Vec<Constraint>,
	#[case] expected: Vec<Range<u16>>,
	#[case] flex: Flex,
) {
	let rect = Rect::new(0, 0, 100, 1);
	let ranges = Layout::horizontal(constraints)
		.flex(flex)
		.split(rect)
		.iter()
		.map(|r| r.left()..r.right())
		.collect_vec();
	assert_eq!(ranges, expected);
}
#[rstest]
#[case::length_overlap1(vec![(0  , 20) , (20 , 20) , (40 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::Start        , 0)]
#[case::length_overlap2(vec![(0  , 20) , (19 , 20) , (38 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::Start        , -1)]
#[case::length_overlap3(vec![(21 , 20) , (40 , 20) , (59 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::Center       , -1)]
#[case::length_overlap4(vec![(42 , 20) , (61 , 20) , (80 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::End          , -1)]
#[case::length_overlap5(vec![(0  , 20) , (19 , 20) , (38 , 62)] , vec![Length(20) , Length(20) , Length(20)] , Flex::Legacy       , -1)]
#[case::length_overlap6(vec![(0  , 20) , (40 , 20) , (80 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::SpaceBetween , -1)]
#[case::length_overlap7(vec![(10 , 20) , (40 , 20) , (70 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::SpaceEvenly  , -1)]
#[case::length_overlap7(vec![(7  , 20) , (40 , 20) , (73 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::SpaceAround  , -1)]
fn flex_overlap(
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
#[case::length_spacing(vec![(0 , 20), (20, 20) , (40, 20)], vec![Length(20), Length(20), Length(20)], Flex::Start      , 0)]
#[case::length_spacing(vec![(0 , 20), (22, 20) , (44, 20)], vec![Length(20), Length(20), Length(20)], Flex::Start      , 2)]
#[case::length_spacing(vec![(18, 20), (40, 20) , (62, 20)], vec![Length(20), Length(20), Length(20)], Flex::Center     , 2)]
#[case::length_spacing(vec![(36, 20), (58, 20) , (80, 20)], vec![Length(20), Length(20), Length(20)], Flex::End        , 2)]
#[case::length_spacing(vec![(0 , 20), (22, 20) , (44, 56)], vec![Length(20), Length(20), Length(20)], Flex::Legacy     , 2)]
#[case::length_spacing(vec![(0 , 20), (40, 20) , (80, 20)], vec![Length(20), Length(20), Length(20)], Flex::SpaceBetween, 2)]
#[case::length_spacing(vec![(10, 20), (40, 20) , (70, 20)], vec![Length(20), Length(20), Length(20)], Flex::SpaceEvenly, 2)]
#[case::length_spacing(vec![(7, 20), (40, 20) , (73, 20)], vec![Length(20), Length(20), Length(20)], Flex::SpaceAround, 2)]
fn flex_spacing(
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
#[case::a(vec![(0, 25), (25, 75)], vec![Length(25), Length(25)])]
#[case::b(vec![(0, 25), (25, 75)], vec![Length(25), Percentage(25)])]
#[case::c(vec![(0, 75), (75, 25)], vec![Percentage(25), Length(25)])]
#[case::d(vec![(0, 75), (75, 25)], vec![Min(25), Percentage(25)])]
#[case::e(vec![(0, 25), (25, 75)], vec![Percentage(25), Min(25)])]
#[case::f(vec![(0, 25), (25, 75)], vec![Min(25), Percentage(100)])]
#[case::g(vec![(0, 75), (75, 25)], vec![Percentage(100), Min(25)])]
#[case::h(vec![(0, 25), (25, 75)], vec![Max(75), Percentage(75)])]
#[case::i(vec![(0, 75), (75, 25)], vec![Percentage(75), Max(75)])]
#[case::j(vec![(0, 25), (25, 75)], vec![Max(25), Percentage(25)])]
#[case::k(vec![(0, 75), (75, 25)], vec![Percentage(25), Max(25)])]
#[case::l(vec![(0, 25), (25, 75)], vec![Length(25), Ratio(1, 4)])]
#[case::m(vec![(0, 75), (75, 25)], vec![Ratio(1, 4), Length(25)])]
#[case::n(vec![(0, 25), (25, 75)], vec![Percentage(25), Ratio(1, 4)])]
#[case::o(vec![(0, 75), (75, 25)], vec![Ratio(1, 4), Percentage(25)])]
#[case::p(vec![(0, 25), (25, 75)], vec![Ratio(1, 4), Fill(25)])]
#[case::q(vec![(0, 75), (75, 25)], vec![Fill(25), Ratio(1, 4)])]
fn constraint_specification_tests_for_priority(
	#[case] expected: Vec<(u16, u16)>,
	#[case] constraints: Vec<Constraint>,
) {
	let rect = Rect::new(0, 0, 100, 1);
	let r = Layout::horizontal(constraints)
		.flex(Flex::Legacy)
		.split(rect)
		.iter()
		.map(|r| (r.x, r.width))
		.collect::<Vec<(u16, u16)>>();
	assert_eq!(r, expected);
}

#[rstest]
#[case::a(vec![(0, 20), (20, 20), (40, 20)], vec![Length(20), Length(20), Length(20)], Flex::Start, 0)]
#[case::b(vec![(18, 20), (40, 20), (62, 20)], vec![Length(20), Length(20), Length(20)], Flex::Center, 2)]
#[case::c(vec![(36, 20), (58, 20), (80, 20)], vec![Length(20), Length(20), Length(20)], Flex::End, 2)]
#[case::d(vec![(0, 20), (22, 20), (44, 56)], vec![Length(20), Length(20), Length(20)], Flex::Legacy, 2)]
#[case::e(vec![(0, 20), (22, 20), (44, 56)], vec![Length(20), Length(20), Length(20)], Flex::Legacy, 2)]
#[case::f(vec![(10, 20), (40, 20), (70, 20)], vec![Length(20), Length(20), Length(20)], Flex::SpaceEvenly, 2)]
#[case::f(vec![(7, 20), (40, 20), (73, 20)], vec![Length(20), Length(20), Length(20)], Flex::SpaceAround, 2)]
fn constraint_specification_tests_for_priority_with_spacing(
	#[case] expected: Vec<(u16, u16)>,
	#[case] constraints: Vec<Constraint>,
	#[case] flex: Flex,
	#[case] spacing: i16,
) {
	let rect = Rect::new(0, 0, 100, 1);
	let r = Layout::horizontal(constraints)
		.spacing(spacing)
		.flex(flex)
		.split(rect)
		.iter()
		.map(|r| (r.x, r.width))
		.collect::<Vec<(u16, u16)>>();
	assert_eq!(r, expected);
}
