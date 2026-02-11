use core::simd::Simd;
use core::simd::cmp::SimdPartialEq;

use super::super::overlapping_load;

#[inline(always)]
pub fn match_haystack_unordered_typos_insensitive(needle: &[(u8, u8)], haystack: &[u8], max_typos: u16) -> bool {
	if max_typos == 0 {
		return super::unordered::match_haystack_unordered_insensitive(needle, haystack);
	}

	let len = haystack.len();

	super::super::super::typos::match_unordered_with_typos(needle.iter().map(|&(c1, c2)| (Simd::splat(c1), Simd::splat(c2))), max_typos, |needle_char| {
		for start in (0..len).step_by(16) {
			let haystack_chunk = overlapping_load(haystack, start, len);
			if haystack_chunk.simd_eq(needle_char.0).any() || haystack_chunk.simd_eq(needle_char.1).any() {
				return true;
			}
		}

		false
	})
}
