//! Ratio constraint tests.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::letters;
use crate::layout::Constraint::{self, *};
use crate::layout::Flex;

#[rstest]
// flex, width, ratios, expected
// Just one ratio takes up the whole space
#[case(Flex::Legacy, 1, &[Ratio(0, 1)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(1, 4)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(1, 2)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(9, 10)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(1, 1)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(2, 1)], "a")]
#[case(Flex::Legacy, 2, &[Ratio(0, 1)], "aa")]
#[case(Flex::Legacy, 2, &[Ratio(1, 10)], "aa")]
#[case(Flex::Legacy, 2, &[Ratio(1, 4)], "aa")]
#[case(Flex::Legacy, 2, &[Ratio(1, 2)], "aa")]
#[case(Flex::Legacy, 2, &[Ratio(2, 3)], "aa")]
#[case(Flex::Legacy, 2, &[Ratio(1, 1)], "aa")]
#[case(Flex::Legacy, 2, &[Ratio(2, 1)], "aa")]
#[case(Flex::Legacy, 1, &[Ratio(0, 1), Ratio(0, 1)], "b")]
#[case(Flex::Legacy, 1, &[Ratio(0, 1), Ratio(1, 10)], "b")]
#[case(Flex::Legacy, 1, &[Ratio(0, 1), Ratio(1, 2)], "b")]
#[case(Flex::Legacy, 1, &[Ratio(0, 1), Ratio(9, 10)], "b")]
#[case(Flex::Legacy, 1, &[Ratio(0, 1), Ratio(1, 1)], "b")]
#[case(Flex::Legacy, 1, &[Ratio(0, 1), Ratio(2, 1)], "b")]
#[case(Flex::Legacy, 1, &[Ratio(1, 10), Ratio(0, 1)], "b")]
#[case(Flex::Legacy, 1, &[Ratio(1, 10), Ratio(1, 10)], "b")]
#[case(Flex::Legacy, 1, &[Ratio(1, 10), Ratio(1, 2)], "b")]
#[case(Flex::Legacy, 1, &[Ratio(1, 10), Ratio(9, 10)], "b")]
#[case(Flex::Legacy, 1, &[Ratio(1, 10), Ratio(1, 1)], "b")]
#[case(Flex::Legacy, 1, &[Ratio(1, 10), Ratio(2, 1)], "b")]
#[case(Flex::Legacy, 1, &[Ratio(1, 2), Ratio(0, 1)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(1, 2), Ratio(1, 2)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(1, 2), Ratio(1, 1)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(1, 2), Ratio(2, 1)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(9, 10), Ratio(0, 1)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(9, 10), Ratio(1, 2)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(9, 10), Ratio(1, 1)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(9, 10), Ratio(2, 1)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(1, 1), Ratio(0, 1)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(1, 1), Ratio(1, 2)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(1, 1), Ratio(1, 1)], "a")]
#[case(Flex::Legacy, 1, &[Ratio(1, 1), Ratio(2, 1)], "a")]
#[case(Flex::Legacy, 2, &[Ratio(0, 1), Ratio(0, 1)], "bb")]
#[case(Flex::Legacy, 2, &[Ratio(0, 1), Ratio(1, 4)], "bb")]
#[case(Flex::Legacy, 2, &[Ratio(0, 1), Ratio(1, 2)], "bb")]
#[case(Flex::Legacy, 2, &[Ratio(0, 1), Ratio(1, 1)], "bb")]
#[case(Flex::Legacy, 2, &[Ratio(0, 1), Ratio(2, 1)], "bb")]
#[case(Flex::Legacy, 2, &[Ratio(1, 10), Ratio(0, 1)], "bb")]
#[case(Flex::Legacy, 2, &[Ratio(1, 10), Ratio(1, 4)], "bb")]
#[case(Flex::Legacy, 2, &[Ratio(1, 10), Ratio(1, 2)], "bb")]
#[case(Flex::Legacy, 2, &[Ratio(1, 10), Ratio(1, 1)], "bb")]
#[case(Flex::Legacy, 2, &[Ratio(1, 10), Ratio(2, 1)], "bb")]
#[case(Flex::Legacy, 2, &[Ratio(1, 4), Ratio(0, 1)], "ab")]
#[case(Flex::Legacy, 2, &[Ratio(1, 4), Ratio(1, 4)], "ab")]
#[case(Flex::Legacy, 2, &[Ratio(1, 4), Ratio(1, 2)], "ab")]
#[case(Flex::Legacy, 2, &[Ratio(1, 4), Ratio(1, 1)], "ab")]
#[case(Flex::Legacy, 2, &[Ratio(1, 4), Ratio(2, 1)], "ab")]
#[case(Flex::Legacy, 2, &[Ratio(1, 3), Ratio(0, 1)], "ab")]
#[case(Flex::Legacy, 2, &[Ratio(1, 3), Ratio(1, 4)], "ab")]
#[case(Flex::Legacy, 2, &[Ratio(1, 3), Ratio(1, 2)], "ab")]
#[case(Flex::Legacy, 2, &[Ratio(1, 3), Ratio(1, 1)], "ab")]
#[case(Flex::Legacy, 2, &[Ratio(1, 3), Ratio(2, 1)], "ab")]
#[case(Flex::Legacy, 2, &[Ratio(1, 2), Ratio(0, 1)], "ab")]
#[case(Flex::Legacy, 2, &[Ratio(1, 2), Ratio(1, 2)], "ab")]
#[case(Flex::Legacy, 2, &[Ratio(1, 2), Ratio(1, 1)], "ab")]
#[case(Flex::Legacy, 2, &[Ratio(1, 1), Ratio(0, 1)], "aa")]
#[case(Flex::Legacy, 2, &[Ratio(1, 1), Ratio(1, 2)], "aa")]
#[case(Flex::Legacy, 2, &[Ratio(1, 1), Ratio(1, 1)], "aa")]
#[case(Flex::Legacy, 3, &[Ratio(1, 3), Ratio(1, 3)], "abb")]
#[case(Flex::Legacy, 3, &[Ratio(1, 3), Ratio(2,3)], "abb")]
#[case(Flex::Legacy, 10, &[Ratio(0, 1), Ratio(0, 1)],  "bbbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(0, 1), Ratio(1, 4)],  "bbbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(0, 1), Ratio(1, 2)],  "bbbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(0, 1), Ratio(1, 1)],  "bbbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(0, 1), Ratio(2, 1)],  "bbbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 10), Ratio(0, 1)], "abbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 10), Ratio(1, 4)], "abbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 10), Ratio(1, 2)], "abbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 10), Ratio(1, 1)], "abbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 10), Ratio(2, 1)], "abbbbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 4), Ratio(0, 1)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 4), Ratio(1, 4)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 4), Ratio(1, 2)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 4), Ratio(1, 1)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 4), Ratio(2, 1)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 3), Ratio(0, 1)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 3), Ratio(1, 4)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 3), Ratio(1, 2)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 3), Ratio(1, 1)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 3), Ratio(2, 1)],  "aaabbbbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 2), Ratio(0, 1)],  "aaaaabbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 2), Ratio(1, 2)],  "aaaaabbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 2), Ratio(1, 1)],  "aaaaabbbbb" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 1), Ratio(0, 1)],  "aaaaaaaaaa" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 1), Ratio(1, 2)],  "aaaaaaaaaa" )]
#[case(Flex::Legacy, 10, &[Ratio(1, 1), Ratio(1, 1)],  "aaaaaaaaaa" )]
fn ratio(
	#[case] flex: Flex,
	#[case] width: u16,
	#[case] constraints: &[Constraint],
	#[case] expected: &str,
) {
	letters(flex, constraints, width, expected);
}

#[rstest]
#[case(Flex::Start, 10, &[Ratio(0, 1), Ratio(0, 1)],   "          " )]
#[case(Flex::Start, 10, &[Ratio(0, 1), Ratio(1, 4)],  "bbb       " )]
#[case(Flex::Start, 10, &[Ratio(0, 1), Ratio(1, 2)],  "bbbbb     " )]
#[case(Flex::Start, 10, &[Ratio(0, 1), Ratio(1, 1)],  "bbbbbbbbbb" )]
#[case(Flex::Start, 10, &[Ratio(0, 1), Ratio(2, 1)],  "bbbbbbbbbb" )]
#[case(Flex::Start, 10, &[Ratio(1, 10), Ratio(0, 1)], "a         " )]
#[case(Flex::Start, 10, &[Ratio(1, 10), Ratio(1, 4)], "abbb      " )]
#[case(Flex::Start, 10, &[Ratio(1, 10), Ratio(1, 2)], "abbbbb    " )]
#[case(Flex::Start, 10, &[Ratio(1, 10), Ratio(1, 1)], "abbbbbbbbb" )]
#[case(Flex::Start, 10, &[Ratio(1, 10), Ratio(2, 1)], "abbbbbbbbb" )]
#[case(Flex::Start, 10, &[Ratio(1, 4), Ratio(0, 1)],  "aaa       " )]
#[case(Flex::Start, 10, &[Ratio(1, 4), Ratio(1, 4)],  "aaabb     " )]
#[case(Flex::Start, 10, &[Ratio(1, 4), Ratio(1, 2)],  "aaabbbbb  " )]
#[case(Flex::Start, 10, &[Ratio(1, 4), Ratio(1, 1)],  "aaabbbbbbb" )]
#[case(Flex::Start, 10, &[Ratio(1, 4), Ratio(2, 1)],  "aaabbbbbbb" )]
#[case(Flex::Start, 10, &[Ratio(1, 3), Ratio(0, 1)],  "aaa       " )]
#[case(Flex::Start, 10, &[Ratio(1, 3), Ratio(1, 4)],  "aaabbb    " )]
#[case(Flex::Start, 10, &[Ratio(1, 3), Ratio(1, 2)],  "aaabbbbb  " )]
#[case(Flex::Start, 10, &[Ratio(1, 3), Ratio(1, 1)],  "aaabbbbbbb" )]
#[case(Flex::Start, 10, &[Ratio(1, 3), Ratio(2, 1)],  "aaabbbbbbb" )]
#[case(Flex::Start, 10, &[Ratio(1, 2), Ratio(0, 1)],  "aaaaa     " )]
#[case(Flex::Start, 10, &[Ratio(1, 2), Ratio(1, 2)],  "aaaaabbbbb" )]
#[case(Flex::Start, 10, &[Ratio(1, 2), Ratio(1, 1)],  "aaaaabbbbb" )]
#[case(Flex::Start, 10, &[Ratio(1, 1), Ratio(0, 1)],  "aaaaaaaaaa" )]
#[case(Flex::Start, 10, &[Ratio(1, 1), Ratio(1, 2)],  "aaaaabbbbb" )]
#[case(Flex::Start, 10, &[Ratio(1, 1), Ratio(1, 1)],  "aaaaabbbbb" )]
#[case(Flex::Start, 10, &[Ratio(1, 1), Ratio(2, 1)],  "aaaaabbbbb" )]
fn ratio_start(
	#[case] flex: Flex,
	#[case] width: u16,
	#[case] constraints: &[Constraint],
	#[case] expected: &str,
) {
	letters(flex, constraints, width, expected);
}

#[rstest]
#[case(Flex::SpaceBetween, 10, &[Ratio(0, 1), Ratio(0, 1)],  "          " )]
#[case(Flex::SpaceBetween, 10, &[Ratio(0, 1), Ratio(1, 4)],  "        bb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(0, 1), Ratio(1, 2)],  "     bbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(0, 1), Ratio(1, 1)],  "bbbbbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(0, 1), Ratio(2, 1)],  "bbbbbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 10), Ratio(0, 1)], "a         " )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 10), Ratio(1, 4)], "a       bb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 10), Ratio(1, 2)], "a    bbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 10), Ratio(1, 1)], "abbbbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 10), Ratio(2, 1)], "abbbbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 4), Ratio(0, 1)],  "aaa       " )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 4), Ratio(1, 4)],  "aaa     bb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 4), Ratio(1, 2)],  "aaa  bbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 4), Ratio(1, 1)],  "aaabbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 4), Ratio(2, 1)],  "aaabbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 3), Ratio(0, 1)],  "aaa       " )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 3), Ratio(1, 4)],  "aaa     bb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 3), Ratio(1, 2)],  "aaa  bbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 3), Ratio(1, 1)],  "aaabbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 3), Ratio(2, 1)],  "aaabbbbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 2), Ratio(0, 1)],  "aaaaa     " )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 2), Ratio(1, 2)],  "aaaaabbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 2), Ratio(1, 1)],  "aaaaabbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 1), Ratio(0, 1)],  "aaaaaaaaaa" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 1), Ratio(1, 2)],  "aaaaabbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 1), Ratio(1, 1)],  "aaaaabbbbb" )]
#[case(Flex::SpaceBetween, 10, &[Ratio(1, 1), Ratio(2, 1)],  "aaaaabbbbb" )]
fn ratio_spacebetween(
	#[case] flex: Flex,
	#[case] width: u16,
	#[case] constraints: &[Constraint],
	#[case] expected: &str,
) {
	letters(flex, constraints, width, expected);
}
