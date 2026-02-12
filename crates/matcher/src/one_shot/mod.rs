pub mod bucket;
mod indices;
mod matcher;
mod parallel;

pub use indices::match_indices;
pub use matcher::match_list;
pub use parallel::match_list_parallel;

pub trait Appendable<T> {
	fn append(&mut self, value: T);
}

impl<T> Appendable<T> for Vec<T> {
	fn append(&mut self, value: T) {
		self.push(value);
	}
}

pub(crate) const MAX_MATRIX_BYTES: usize = 32 * 1024; // 32 KB
/// Deterministic typo-mode SW compute budget.
/// Keeps fallback behavior stable across SIMD lane widths and avoids expensive typo SW on very wide buckets.
pub(crate) const MAX_TYPO_SW_CELLS: usize = 2 * 1024;
#[inline(always)]
pub(crate) fn match_too_large(needle: &str, haystack: &str) -> bool {
	let max_haystack_len = MAX_MATRIX_BYTES / needle.len().max(1) / 2; // divide by 2 since we use u16
	haystack.len() > max_haystack_len
}

#[inline(always)]
pub(crate) fn typo_sw_too_large(needle: &str, haystack_width: usize) -> bool {
	needle.len().saturating_mul(haystack_width) > MAX_TYPO_SW_CELLS
}

#[inline(always)]
pub(crate) fn exceeds_typo_budget(max_typos: Option<u16>, needle: &str, matched_needle_chars: usize) -> bool {
	max_typos.is_some_and(|max_typos| needle.len().saturating_sub(matched_needle_chars) > max_typos as usize)
}
