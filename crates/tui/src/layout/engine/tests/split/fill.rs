//! Fill constraint tests.

use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;

use itertools::Itertools;
use pretty_assertions::assert_eq;
use rstest::rstest;

use crate::layout::Constraint::{self, *};
use crate::layout::{Flex, Layout, Rect};

#[rstest]
#[case::fill_len_fill(vec![Fill(1), Length(10), Fill(2)], vec![0..13, 13..23, 23..50])]
#[case::len_fill_fill(vec![Length(10), Fill(2), Fill(1)], vec![0..10, 10..37, 37..50])] // might be unstable?
fn fixed_with_50_width(#[case] constraints: Vec<Constraint>, #[case] expected: Vec<Range<u16>>) {
	let rect = Rect::new(0, 0, 50, 1);
	let ranges = Layout::horizontal(constraints)
		.flex(Flex::Legacy)
		.split(rect)
		.iter()
		.map(|r| r.left()..r.right())
		.collect_vec();
	assert_eq!(ranges, expected);
}

#[rstest]
#[case::same_fill(vec![Fill(1), Fill(2), Fill(1), Fill(1)], vec![0..20, 20..60, 60..80, 80..100])]
#[case::inc_fill(vec![Fill(1), Fill(2), Fill(3), Fill(4)], vec![0..10, 10..30, 30..60, 60..100])]
#[case::dec_fill(vec![Fill(4), Fill(3), Fill(2), Fill(1)], vec![0..40, 40..70, 70..90, 90..100])]
#[case::rand_fill1(vec![Fill(1), Fill(3), Fill(2), Fill(4)], vec![0..10, 10..40, 40..60, 60..100])]
#[case::rand_fill2(vec![Fill(1), Fill(3), Length(50), Fill(2), Fill(4)], vec![0..5, 5..20, 20..70, 70..80, 80..100])]
#[case::rand_fill3(vec![Fill(1), Fill(3), Percentage(50), Fill(2), Fill(4)], vec![0..5, 5..20, 20..70, 70..80, 80..100])]
#[case::rand_fill4(vec![Fill(1), Fill(3), Min(50), Fill(2), Fill(4)], vec![0..5, 5..20, 20..70, 70..80, 80..100])]
#[case::rand_fill5(vec![Fill(1), Fill(3), Max(50), Fill(2), Fill(4)], vec![0..5, 5..20, 20..70, 70..80, 80..100])]
#[case::zero_fill1(vec![Fill(0), Fill(1), Fill(0)], vec![0..0, 0..100, 100..100])]
#[case::zero_fill2(vec![Fill(0), Length(1), Fill(0)], vec![0..50, 50..51, 51..100])]
#[case::zero_fill3(vec![Fill(0), Percentage(1), Fill(0)], vec![0..50, 50..51, 51..100])]
#[case::zero_fill4(vec![Fill(0), Min(1), Fill(0)], vec![0..50, 50..51, 51..100])]
#[case::zero_fill5(vec![Fill(0), Max(1), Fill(0)], vec![0..50, 50..51, 51..100])]
#[case::zero_fill6(vec![Fill(0), Fill(2), Fill(0), Fill(1)], vec![0..0, 0..67, 67..67, 67..100])]
#[case::space_fill1(vec![Fill(0), Fill(2), Percentage(20)], vec![0..0, 0..80, 80..100])]
#[case::space_fill2(vec![Fill(0), Fill(0), Percentage(20)], vec![0..40, 40..80, 80..100])]
#[case::space_fill3(vec![Fill(0), Ratio(1, 5)], vec![0..80, 80..100])]
#[case::space_fill4(vec![Fill(0), Fill(u16::MAX)], vec![0..0, 0..100])]
#[case::space_fill5(vec![Fill(u16::MAX), Fill(0)], vec![0..100, 100..100])]
#[case::space_fill6(vec![Fill(0), Percentage(20)], vec![0..80, 80..100])]
#[case::space_fill7(vec![Fill(1), Percentage(20)], vec![0..80, 80..100])]
#[case::space_fill8(vec![Fill(u16::MAX), Percentage(20)], vec![0..80, 80..100])]
#[case::space_fill9(vec![Fill(u16::MAX), Fill(0), Percentage(20)], vec![0..80, 80..80, 80..100])]
#[case::space_fill10(vec![Fill(0), Length(20)], vec![0..80, 80..100])]
#[case::space_fill11(vec![Fill(0), Min(20)], vec![0..80, 80..100])]
#[case::space_fill12(vec![Fill(0), Max(20)], vec![0..80, 80..100])]
#[case::fill_collapse1(vec![Fill(1), Fill(1), Fill(1), Min(30), Length(50)], vec![0..7, 7..13, 13..20, 20..50, 50..100])]
#[case::fill_collapse2(vec![Fill(1), Fill(1), Fill(1), Length(50), Length(50)], vec![0..0, 0..0, 0..0, 0..50, 50..100])]
#[case::fill_collapse3(vec![Fill(1), Fill(1), Fill(1), Length(75), Length(50)], vec![0..0, 0..0, 0..0, 0..75, 75..100])]
#[case::fill_collapse4(vec![Fill(1), Fill(1), Fill(1), Min(50), Max(50)], vec![0..0, 0..0, 0..0, 0..50, 50..100])]
#[case::fill_collapse5(vec![Fill(1), Fill(1), Fill(1), Ratio(1, 1)], vec![0..0, 0..0, 0..0, 0..100])]
#[case::fill_collapse6(vec![Fill(1), Fill(1), Fill(1), Percentage(100)], vec![0..0, 0..0, 0..0, 0..100])]
fn fill(#[case] constraints: Vec<Constraint>, #[case] expected: Vec<Range<u16>>) {
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
#[case::prop(vec![(0 , 10), (10, 80), (90 , 10)] , vec![Length(10), Fill(1), Length(10)], Flex::Legacy)]
#[case::flex(vec![(0 , 10), (90 , 10)] , vec![Length(10), Length(10)], Flex::SpaceBetween)]
#[case::prop(vec![(0 , 27), (27, 10), (37, 26), (63, 10), (73, 27)] , vec![Fill(1), Length(10), Fill(1), Length(10), Fill(1)], Flex::Legacy)]
#[case::flex(vec![(27 , 10), (63, 10)] , vec![Length(10), Length(10)], Flex::SpaceEvenly)]
#[case::prop(vec![(0 , 10), (10, 10), (20 , 80)] , vec![Length(10), Length(10), Fill(1)], Flex::Legacy)]
#[case::flex(vec![(0 , 10), (10, 10)] , vec![Length(10), Length(10)], Flex::Start)]
#[case::prop(vec![(0 , 80), (80 , 10), (90, 10)] , vec![Fill(1), Length(10), Length(10)], Flex::Legacy)]
#[case::flex(vec![(80 , 10), (90, 10)] , vec![Length(10), Length(10)], Flex::End)]
#[case::prop(vec![(0 , 40), (40, 10), (50, 10), (60, 40)] , vec![Fill(1), Length(10), Length(10), Fill(1)], Flex::Legacy)]
#[case::flex(vec![(40 , 10), (50, 10)] , vec![Length(10), Length(10)], Flex::Center)]
#[case::flex(vec![(20 , 10), (70, 10)] , vec![Length(10), Length(10)], Flex::SpaceAround)]
fn fill_vs_flex(
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

#[rstest]
#[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::Legacy , 0)]
#[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceEvenly , 0)]
#[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceBetween , 0)]
#[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceAround , 0)]
#[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::Start , 0)]
#[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::Center , 0)]
#[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::End , 0)]
#[case::flex10(vec![(0 , 45), (55 , 45)] , vec![Fill(1), Fill(1)], Flex::Legacy , 10)]
#[case::flex10(vec![(0 , 45), (55 , 45)] , vec![Fill(1), Fill(1)], Flex::Start , 10)]
#[case::flex10(vec![(0 , 45), (55 , 45)] , vec![Fill(1), Fill(1)], Flex::Center , 10)]
#[case::flex10(vec![(0 , 45), (55 , 45)] , vec![Fill(1), Fill(1)], Flex::End , 10)]
#[case::flex10(vec![(10 , 35), (55 , 35)] , vec![Fill(1), Fill(1)], Flex::SpaceEvenly , 10)]
#[case::flex10(vec![(0 , 45), (55 , 45)] , vec![Fill(1), Fill(1)], Flex::SpaceBetween , 10)]
#[case::flex10(vec![(10 , 30), (60 , 30)] , vec![Fill(1), Fill(1)], Flex::SpaceAround , 10)]
#[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::Legacy , 0)]
#[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceEvenly , 0)]
#[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceBetween , 0)]
#[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceAround , 0)]
#[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::Start , 0)]
#[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::Center , 0)]
#[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::End , 0)]
#[case::flex_length10(vec![(0 , 35), (45, 10), (65 , 35)] , vec![Fill(1), Length(10), Fill(1)], Flex::Legacy , 10)]
#[case::flex_length10(vec![(0 , 35), (45, 10), (65 , 35)] , vec![Fill(1), Length(10), Fill(1)], Flex::Start , 10)]
#[case::flex_length10(vec![(0 , 35), (45, 10), (65 , 35)] , vec![Fill(1), Length(10), Fill(1)], Flex::Center , 10)]
#[case::flex_length10(vec![(0 , 35), (45, 10), (65 , 35)] , vec![Fill(1), Length(10), Fill(1)], Flex::End , 10)]
#[case::flex_length10(vec![(10 , 25), (45, 10), (65 , 25)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceEvenly , 10)]
#[case::flex_length10(vec![(0 , 35), (45, 10), (65 , 35)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceBetween , 10)]
#[case::flex_length10(vec![(10 , 15), (45, 10), (75 , 15)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceAround , 10)]
fn fill_spacing(
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
	assert_eq!(expected, result);
}

#[rstest]
#[case::flex0_1(vec![(0 , 55), (45 , 55)] , vec![Fill(1), Fill(1)], Flex::Legacy , -10)]
#[case::flex0_2(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceAround , -10)]
#[case::flex0_3(vec![(0 , 55), (45 , 55)] , vec![Fill(1), Fill(1)], Flex::SpaceBetween , -10)]
#[case::flex0_4(vec![(0 , 55), (45 , 55)] , vec![Fill(1), Fill(1)], Flex::Start , -10)]
#[case::flex0_5(vec![(0 , 55), (45 , 55)] , vec![Fill(1), Fill(1)], Flex::Center , -10)]
#[case::flex0_6(vec![(0 , 55), (45 , 55)] , vec![Fill(1), Fill(1)], Flex::End , -10)]
#[case::flex0_7(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceEvenly , -10)]
#[case::flex10_1(vec![(0 , 51), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::Legacy , -1)]
#[case::flex10_2(vec![(0 , 51), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::Start , -1)]
#[case::flex10_3(vec![(0 , 51), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::Center , -1)]
#[case::flex10_4(vec![(0 , 51), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::End , -1)]
#[case::flex10_5(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceAround , -1)]
#[case::flex10_6(vec![(0 , 51), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceBetween , -1)]
#[case::flex10_7(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceEvenly , -1)]
#[case::flex_length0_1(vec![(0 , 55), (45, 10), (45 , 55)] , vec![Fill(1), Length(10), Fill(1)], Flex::Legacy , -10)]
#[case::flex_length0_2(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceAround , -10)]
#[case::flex_length0_3(vec![(0 , 55), (45, 10), (45 , 55)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceBetween , -10)]
#[case::flex_length0_4(vec![(0 , 55), (45, 10), (45 , 55)] , vec![Fill(1), Length(10), Fill(1)], Flex::Start , -10)]
#[case::flex_length0_5(vec![(0 , 55), (45, 10), (45 , 55)] , vec![Fill(1), Length(10), Fill(1)], Flex::Center , -10)]
#[case::flex_length0_6(vec![(0 , 55), (45, 10), (45 , 55)] , vec![Fill(1), Length(10), Fill(1)], Flex::End , -10)]
#[case::flex_length0_7(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceEvenly , -10)]
#[case::flex_length10_1(vec![(0 , 46), (45, 10), (54 , 46)] , vec![Fill(1), Length(10), Fill(1)], Flex::Legacy , -1)]
#[case::flex_length10_2(vec![(0 , 46), (45, 10), (54 , 46)] , vec![Fill(1), Length(10), Fill(1)], Flex::Start , -1)]
#[case::flex_length10_3(vec![(0 , 46), (45, 10), (54 , 46)] , vec![Fill(1), Length(10), Fill(1)], Flex::Center , -1)]
#[case::flex_length10_4(vec![(0 , 46), (45, 10), (54 , 46)] , vec![Fill(1), Length(10), Fill(1)], Flex::End , -1)]
#[case::flex_length10_5(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceAround , -1)]
#[case::flex_length10_6(vec![(0 , 46), (45, 10), (54 , 46)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceBetween , -1)]
#[case::flex_length10_7(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceEvenly , -1)]
fn fill_overlap(
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
