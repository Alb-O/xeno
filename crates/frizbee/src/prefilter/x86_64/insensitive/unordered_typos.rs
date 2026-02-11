use std::arch::x86_64::*;

use super::super::overlapping_load;

/// Checks if the needle is wholly contained in the haystack, ignoring the exact order of the
/// bytes. For example, if the needle is "test", the haystack "tset" will return true.
/// The needle chars must include both the uppercase and lowercase variants of the character.
///
/// Fastest with SSE2, AVX, and AVX2, but still very fast with just SSE2. Use a function with
/// `#[target_feature(enable = "sse2,avx,avx2")]` or `#[target_feature(enable = "sse2")]`
///
/// # Safety
/// When W > 16, the caller must ensure that the minimum length of the haystack is >= 16.
/// When W <= 16, the caller must ensure that the minimum length of the haystack is >= 8.
/// In all cases, the caller must ensure SSE2 is available.
#[inline(always)]
pub unsafe fn match_haystack_unordered_typos_insensitive(needle: &[(u8, u8)], haystack: &[u8], max_typos: u16) -> bool {
	if max_typos == 0 {
		return unsafe { super::unordered::match_haystack_unordered_insensitive(needle, haystack) };
	}

	let len = haystack.len();

	super::super::super::typos::match_unordered_with_typos(
		needle.iter().map(|&(c1, c2)| unsafe { (_mm_set1_epi8(c1 as i8), _mm_set1_epi8(c2 as i8)) }),
		max_typos,
		|needle_char| {
			for start in (0..len).step_by(16) {
				let haystack_chunk = unsafe { overlapping_load(haystack, start, len) };
				if unsafe { _mm_movemask_epi8(_mm_cmpeq_epi8(needle_char.0, haystack_chunk)) } != 0
					|| unsafe { _mm_movemask_epi8(_mm_cmpeq_epi8(needle_char.1, haystack_chunk)) } != 0
				{
					return true;
				}
			}

			false
		},
	)
}

/// Checks if the needle is wholly contained in the haystack, ignoring the exact order of the
/// bytes. For example, if the needle is "test", the haystack "tset" will return true.
/// The needle chars must include both the uppercase and lowercase variants of the character.
///
/// Use a function with `#[target_feature(enable = "sse2,avx,avx2")]`
///
/// # Safety
/// When W > 16, the caller must ensure that the minimum length of the haystack is >= 16.
/// When W <= 16, the caller must ensure that the minimum length of the haystack is >= 8.
/// In all cases, the caller must ensure SSE2 and AVX2 are available.
#[inline(always)]
pub unsafe fn match_haystack_unordered_typos_insensitive_avx2(needle: &[__m256i], haystack: &[u8], max_typos: u16) -> bool {
	if max_typos == 0 {
		return unsafe { super::unordered::match_haystack_unordered_insensitive_avx2(needle, haystack) };
	}

	let len = haystack.len();

	super::super::super::typos::match_unordered_with_typos(needle.iter().copied(), max_typos, |needle_char| {
		for start in (0..len).step_by(16) {
			let haystack_chunk = unsafe { overlapping_load(haystack, start, len) };
			let haystack_chunk = unsafe { _mm256_broadcastsi128_si256(haystack_chunk) };

			if unsafe { _mm256_movemask_epi8(_mm256_cmpeq_epi8(needle_char, haystack_chunk)) } != 0 {
				return true;
			}
		}

		false
	})
}
