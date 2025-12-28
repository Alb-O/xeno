//! Percentage constraint tests.

use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;

use itertools::Itertools;
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::letters;
use crate::layout::Constraint::{self, *};
use crate::layout::{Flex, Layout, Rect};

#[rstest] // flex, width, lengths, expected
// One constraint will take all the space (width = 1)
#[case(Flex::Legacy, 1, &[Percentage(0)],   "a")]
#[case(Flex::Legacy, 1, &[Percentage(25)],  "a")]
#[case(Flex::Legacy, 1, &[Percentage(50)],  "a")]
#[case(Flex::Legacy, 1, &[Percentage(90)],  "a")]
#[case(Flex::Legacy, 1, &[Percentage(100)], "a")]
#[case(Flex::Legacy, 1, &[Percentage(200)], "a")]
// One constraint will take all the space (width = 2)
#[case(Flex::Legacy, 2, &[Percentage(0)],   "aa")]
#[case(Flex::Legacy, 2, &[Percentage(10)],  "aa")]
#[case(Flex::Legacy, 2, &[Percentage(25)],  "aa")]
#[case(Flex::Legacy, 2, &[Percentage(50)],  "aa")]
#[case(Flex::Legacy, 2, &[Percentage(66)],  "aa")]
#[case(Flex::Legacy, 2, &[Percentage(100)], "aa")]
#[case(Flex::Legacy, 2, &[Percentage(200)], "aa")]
// One constraint will take all the space (width = 3)
#[case(Flex::Legacy, 10, &[Percentage(0)],   "aaaaaaaaaa")]
#[case(Flex::Legacy, 10, &[Percentage(10)],  "aaaaaaaaaa")]
#[case(Flex::Legacy, 10, &[Percentage(25)],  "aaaaaaaaaa")]
#[case(Flex::Legacy, 10, &[Percentage(50)],  "aaaaaaaaaa")]
#[case(Flex::Legacy, 10, &[Percentage(66)],  "aaaaaaaaaa")]
#[case(Flex::Legacy, 10, &[Percentage(100)], "aaaaaaaaaa")]
#[case(Flex::Legacy, 10, &[Percentage(200)], "aaaaaaaaaa")]
// 0%/any allocates all the space to the second constraint
#[case(Flex::Legacy, 1, &[Percentage(0), Percentage(0)],   "b")]
#[case(Flex::Legacy, 1, &[Percentage(0), Percentage(10)],  "b")]
#[case(Flex::Legacy, 1, &[Percentage(0), Percentage(50)],  "b")]
#[case(Flex::Legacy, 1, &[Percentage(0), Percentage(90)],  "b")]
#[case(Flex::Legacy, 1, &[Percentage(0), Percentage(100)], "b")]
#[case(Flex::Legacy, 1, &[Percentage(0), Percentage(200)], "b")]
// 10%/any allocates all the space to the second constraint (even if it is 0)
#[case(Flex::Legacy, 1, &[Percentage(10), Percentage(0)],   "b")]
#[case(Flex::Legacy, 1, &[Percentage(10), Percentage(10)],  "b")]
#[case(Flex::Legacy, 1, &[Percentage(10), Percentage(50)],  "b")]
#[case(Flex::Legacy, 1, &[Percentage(10), Percentage(90)],  "b")]
#[case(Flex::Legacy, 1, &[Percentage(10), Percentage(100)], "b")]
#[case(Flex::Legacy, 1, &[Percentage(10), Percentage(200)], "b")]
// 50%/any allocates all the space to the first constraint
#[case(Flex::Legacy, 1, &[Percentage(50), Percentage(0)],   "a")]
#[case(Flex::Legacy, 1, &[Percentage(50), Percentage(50)],  "a")]
#[case(Flex::Legacy, 1, &[Percentage(50), Percentage(100)], "a")]
#[case(Flex::Legacy, 1, &[Percentage(50), Percentage(200)], "a")]
// 90%/any allocates all the space to the first constraint
#[case(Flex::Legacy, 1, &[Percentage(90), Percentage(0)],   "a")]
#[case(Flex::Legacy, 1, &[Percentage(90), Percentage(50)],  "a")]
#[case(Flex::Legacy, 1, &[Percentage(90), Percentage(100)], "a")]
#[case(Flex::Legacy, 1, &[Percentage(90), Percentage(200)], "a")]
// 100%/any allocates all the space to the first constraint
#[case(Flex::Legacy, 1, &[Percentage(100), Percentage(0)],   "a")]
#[case(Flex::Legacy, 1, &[Percentage(100), Percentage(50)],  "a")]
#[case(Flex::Legacy, 1, &[Percentage(100), Percentage(100)], "a")]
#[case(Flex::Legacy, 1, &[Percentage(100), Percentage(200)], "a")]
// 0%/any allocates all the space to the second constraint
#[case(Flex::Legacy, 2, &[Percentage(0), Percentage(0)],   "bb")]
#[case(Flex::Legacy, 2, &[Percentage(0), Percentage(25)],  "bb")]
#[case(Flex::Legacy, 2, &[Percentage(0), Percentage(50)],  "bb")]
#[case(Flex::Legacy, 2, &[Percentage(0), Percentage(100)], "bb")]
#[case(Flex::Legacy, 2, &[Percentage(0), Percentage(200)], "bb")]
// 10%/any allocates all the space to the second constraint
#[case(Flex::Legacy, 2, &[Percentage(10), Percentage(0)],   "bb")]
#[case(Flex::Legacy, 2, &[Percentage(10), Percentage(25)],  "bb")]
#[case(Flex::Legacy, 2, &[Percentage(10), Percentage(50)],  "bb")]
#[case(Flex::Legacy, 2, &[Percentage(10), Percentage(100)], "bb")]
#[case(Flex::Legacy, 2, &[Percentage(10), Percentage(200)], "bb")]
// 25% * 2 = 0.5, which rounds up to 1, so the first constraint gets 1
#[case(Flex::Legacy, 2, &[Percentage(25), Percentage(0)],   "ab")]
#[case(Flex::Legacy, 2, &[Percentage(25), Percentage(25)],  "ab")]
#[case(Flex::Legacy, 2, &[Percentage(25), Percentage(50)],  "ab")]
#[case(Flex::Legacy, 2, &[Percentage(25), Percentage(100)], "ab")]
#[case(Flex::Legacy, 2, &[Percentage(25), Percentage(200)], "ab")]
// 33% * 2 = 0.66, so the first constraint gets 1
#[case(Flex::Legacy, 2, &[Percentage(33), Percentage(0)],   "ab")]
#[case(Flex::Legacy, 2, &[Percentage(33), Percentage(25)],  "ab")]
#[case(Flex::Legacy, 2, &[Percentage(33), Percentage(50)],  "ab")]
#[case(Flex::Legacy, 2, &[Percentage(33), Percentage(100)], "ab")]
#[case(Flex::Legacy, 2, &[Percentage(33), Percentage(200)], "ab")]
// 50% * 2 = 1, so the first constraint gets 1
#[case(Flex::Legacy, 2, &[Percentage(50), Percentage(0)],   "ab")]
#[case(Flex::Legacy, 2, &[Percentage(50), Percentage(50)],  "ab")]
#[case(Flex::Legacy, 2, &[Percentage(50), Percentage(100)], "ab")]
// 100%/any allocates all the space to the first constraint
// This is probably not the correct behavior, but it is the current behavior
#[case(Flex::Legacy, 2, &[Percentage(100), Percentage(0)],   "aa")]
#[case(Flex::Legacy, 2, &[Percentage(100), Percentage(50)],  "aa")]
#[case(Flex::Legacy, 2, &[Percentage(100), Percentage(100)], "aa")]
// 33%/any allocates 1 to the first constraint the rest to the second
#[case(Flex::Legacy, 3, &[Percentage(33), Percentage(33)], "abb")]
#[case(Flex::Legacy, 3, &[Percentage(33), Percentage(66)], "abb")]
// 33%/any allocates 1.33 = 1 to the first constraint the rest to the second
#[case(Flex::Legacy, 4, &[Percentage(33), Percentage(33)], "abbb")]
#[case(Flex::Legacy, 4, &[Percentage(33), Percentage(66)], "abbb")]
// Longer tests zero allocates everything to the second constraint
#[case(Flex::Legacy, 10, &[Percentage(0),   Percentage(0)],   "bbbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(0),   Percentage(25)],  "bbbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(0),   Percentage(50)],  "bbbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(0),   Percentage(100)], "bbbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(0),   Percentage(200)], "bbbbbbbbbb" )]
// 10% allocates a single character to the first constraint
#[case(Flex::Legacy, 10, &[Percentage(10),  Percentage(0)],   "abbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(10),  Percentage(25)],  "abbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(10),  Percentage(50)],  "abbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(10),  Percentage(100)], "abbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(10),  Percentage(200)], "abbbbbbbbb" )]
// 25% allocates 2.5 = 3 characters to the first constraint
#[case(Flex::Legacy, 10, &[Percentage(25),  Percentage(0)],   "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(25),  Percentage(25)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(25),  Percentage(50)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(25),  Percentage(100)], "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(25),  Percentage(200)], "aaabbbbbbb" )]
// 33% allocates 3.3 = 3 characters to the first constraint
#[case(Flex::Legacy, 10, &[Percentage(33),  Percentage(0)],   "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(33),  Percentage(25)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(33),  Percentage(50)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(33),  Percentage(100)], "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(33),  Percentage(200)], "aaabbbbbbb" )]
// 50% allocates 5 characters to the first constraint
#[case(Flex::Legacy, 10, &[Percentage(50),  Percentage(0)],   "aaaaabbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(50),  Percentage(50)],  "aaaaabbbbb" )]
#[case(Flex::Legacy, 10, &[Percentage(50),  Percentage(100)], "aaaaabbbbb" )]
// 100% allocates everything to the first constraint
#[case(Flex::Legacy, 10, &[Percentage(100), Percentage(0)],   "aaaaaaaaaa" )]
#[case(Flex::Legacy, 10, &[Percentage(100), Percentage(50)],  "aaaaaaaaaa" )]
#[case(Flex::Legacy, 10, &[Percentage(100), Percentage(100)], "aaaaaaaaaa" )]
fn percentage(
	#[case] flex: Flex,
	#[case] width: u16,
	#[case] constraints: &[Constraint],
	#[case] expected: &str,
) {
	letters(flex, constraints, width, expected);
}

#[rstest]
#[case(Flex::Start, 10, &[Percentage(0),   Percentage(0)],    "          " )]
#[case(Flex::Start, 10, &[Percentage(0),   Percentage(25)],  "bbb       " )]
#[case(Flex::Start, 10, &[Percentage(0),   Percentage(50)],  "bbbbb     " )]
#[case(Flex::Start, 10, &[Percentage(0),   Percentage(100)], "bbbbbbbbbb" )]
#[case(Flex::Start, 10, &[Percentage(0),   Percentage(200)], "bbbbbbbbbb" )]
#[case(Flex::Start, 10, &[Percentage(10),  Percentage(0)],   "a         " )]
#[case(Flex::Start, 10, &[Percentage(10),  Percentage(25)],  "abbb      " )]
#[case(Flex::Start, 10, &[Percentage(10),  Percentage(50)],  "abbbbb    " )]
#[case(Flex::Start, 10, &[Percentage(10),  Percentage(100)], "abbbbbbbbb" )]
#[case(Flex::Start, 10, &[Percentage(10),  Percentage(200)], "abbbbbbbbb" )]
#[case(Flex::Start, 10, &[Percentage(25),  Percentage(0)],   "aaa       " )]
#[case(Flex::Start, 10, &[Percentage(25),  Percentage(25)],  "aaabb     " )]
#[case(Flex::Start, 10, &[Percentage(25),  Percentage(50)],  "aaabbbbb  " )]
#[case(Flex::Start, 10, &[Percentage(25),  Percentage(100)], "aaabbbbbbb" )]
#[case(Flex::Start, 10, &[Percentage(25),  Percentage(200)], "aaabbbbbbb" )]
#[case(Flex::Start, 10, &[Percentage(33),  Percentage(0)],   "aaa       " )]
#[case(Flex::Start, 10, &[Percentage(33),  Percentage(25)],  "aaabbb    " )]
#[case(Flex::Start, 10, &[Percentage(33),  Percentage(50)],  "aaabbbbb  " )]
#[case(Flex::Start, 10, &[Percentage(33),  Percentage(100)], "aaabbbbbbb" )]
#[case(Flex::Start, 10, &[Percentage(33),  Percentage(200)], "aaabbbbbbb" )]
#[case(Flex::Start, 10, &[Percentage(50),  Percentage(0)],   "aaaaa     " )]
#[case(Flex::Start, 10, &[Percentage(50),  Percentage(50)],  "aaaaabbbbb" )]
#[case(Flex::Start, 10, &[Percentage(50),  Percentage(100)], "aaaaabbbbb" )]
#[case(Flex::Start, 10, &[Percentage(100), Percentage(0)],   "aaaaaaaaaa" )]
#[case(Flex::Start, 10, &[Percentage(100), Percentage(50)],  "aaaaabbbbb" )]
#[case(Flex::Start, 10, &[Percentage(100), Percentage(100)], "aaaaabbbbb" )]
#[case(Flex::Start, 10, &[Percentage(100), Percentage(200)], "aaaaabbbbb" )]
fn percentage_start(
	#[case] flex: Flex,
	#[case] width: u16,
	#[case] constraints: &[Constraint],
	#[case] expected: &str,
) {
	letters(flex, constraints, width, expected);
}

#[rstest]
#[case(Flex::SpaceBetween, 10, &[Percentage(0),   Percentage(0)],   "          " )]
#[case(Flex::SpaceBetween, 10, &[Percentage(0),   Percentage(25)],  "        bb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(0),   Percentage(50)],  "     bbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(0),   Percentage(100)], "bbbbbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(0),   Percentage(200)], "bbbbbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(10),  Percentage(0)],   "a         " )]
#[case(Flex::SpaceBetween, 10, &[Percentage(10),  Percentage(25)],  "a       bb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(10),  Percentage(50)],  "a    bbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(10),  Percentage(100)], "abbbbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(10),  Percentage(200)], "abbbbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(25),  Percentage(0)],   "aaa       " )]
#[case(Flex::SpaceBetween, 10, &[Percentage(25),  Percentage(25)],  "aaa     bb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(25),  Percentage(50)],  "aaa  bbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(25),  Percentage(100)], "aaabbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(25),  Percentage(200)], "aaabbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(33),  Percentage(0)],   "aaa       " )]
#[case(Flex::SpaceBetween, 10, &[Percentage(33),  Percentage(25)],  "aaa     bb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(33),  Percentage(50)],  "aaa  bbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(33),  Percentage(100)], "aaabbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(33),  Percentage(200)], "aaabbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(50),  Percentage(0)],   "aaaaa     " )]
#[case(Flex::SpaceBetween, 10, &[Percentage(50),  Percentage(50)],  "aaaaabbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(50),  Percentage(100)], "aaaaabbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(100), Percentage(0)],   "aaaaaaaaaa" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(100), Percentage(50)],  "aaaaabbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(100), Percentage(100)], "aaaaabbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Percentage(100), Percentage(200)], "aaaaabbbbb" )]
fn percentage_spacebetween(
	#[case] flex: Flex,
	#[case] width: u16,
	#[case] constraints: &[Constraint],
	#[case] expected: &str,
) {
	letters(flex, constraints, width, expected);
}
#[rstest]
#[case::min_percentage(vec![Min(0), Percentage(20)], vec![0..80, 80..100])]
#[case::max_percentage(vec![Max(0), Percentage(20)], vec![0..0, 0..100])]
fn percentage_parameterized(
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
