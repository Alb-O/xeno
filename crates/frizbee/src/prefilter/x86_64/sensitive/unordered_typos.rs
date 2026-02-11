use std::arch::x86_64::*;

use super::super::overlapping_load;

/// Checks if the needle is wholly contained in the haystack, ignoring the exact order of the
/// bytes. For example, if the needle is "test", the haystack "tset" will return true.
///
/// Fastest with SSE2, AVX, and AVX2, but still very fast with just SSE2. Use a function with
/// `#[target_feature(enable = "sse2,avx,avx2")]` or `#[target_feature(enable = "sse2")]`
///
/// # Safety
/// When W > 16, the caller must ensure that the minimum length of the haystack is >= 16.
/// When W <= 16, the caller must ensure that the minimum length of the haystack is >= 8.
/// In all cases, the caller must ensure SSE2 is available.
#[inline(always)]
pub unsafe fn match_haystack_unordered_typos(needle: &[u8], haystack: &[u8], max_typos: u16) -> bool {
	if max_typos == 0 {
		return unsafe { super::unordered::match_haystack_unordered(needle, haystack) };
	}

	let len = haystack.len();

	super::super::super::typos::match_unordered_with_typos(needle.iter().map(|&c| unsafe { _mm_set1_epi8(c as i8) }), max_typos, |needle_char| {
		for start in (0..len).step_by(16) {
			let haystack_chunk = unsafe { overlapping_load(haystack, start, len) };
			let cmp = unsafe { _mm_cmpeq_epi8(needle_char, haystack_chunk) };
			if unsafe { _mm_movemask_epi8(cmp) } != 0 {
				return true;
			}
		}

		false
	})
}
