//! Fast prefiltering algorithms, which run before Smith Waterman since in the typical case,
//! a small percentage of the haystack will match the needle. Automatically used by the Matcher
//! and match_list APIs.
//!
//! Unordered algorithms are much faster than ordered algorithms, but don't guarantee that the
//! needle is contained in the haystack, unlike ordered algorithms. As a result, a backwards
//! pass must be performed after Smith Waterman to verify the number of typos. But the faster
//! prefilter generally seems to outweigh this extra cost.
//!
//! The `Prefilter` struct chooses the fastest algorithm via runtime feature detection.
//!
//! All algorithms, except scalar, assume that needle.len() > 0 && haystack.len() >= 8

pub mod bitmask;
pub mod scalar;
pub mod simd;
#[cfg(target_arch = "x86_64")]
pub mod x86_64;

#[derive(Clone, Debug)]
pub struct Prefilter {
	needle: String,
	needle_cased: Vec<(u8, u8)>,
	#[cfg(target_arch = "x86_64")]
	needle_avx2: Option<Vec<std::arch::x86_64::__m256i>>,

	max_typos: u16,

	has_sse2: bool,
	has_avx2: bool,
}

impl Prefilter {
	pub fn new(needle: &str, max_typos: u16) -> Self {
		#[cfg(target_arch = "x86_64")]
		let has_sse2 = is_x86_feature_detected!("sse2");
		#[cfg(not(target_arch = "x86_64"))]
		let has_sse2 = false;

		#[cfg(target_arch = "x86_64")]
		let has_avx2 = has_sse2 && is_x86_feature_detected!("avx2") && is_x86_feature_detected!("avx");
		#[cfg(not(target_arch = "x86_64"))]
		let has_avx2 = false;

		let needle_cased = Self::case_needle(needle);
		Prefilter {
			needle: needle.to_string(),
			needle_cased: needle_cased.clone(),
			#[cfg(target_arch = "x86_64")]
			needle_avx2: has_avx2.then(|| unsafe { x86_64::needle_to_avx2(&needle_cased) }),

			max_typos,

			has_sse2,
			has_avx2,
		}
	}

	pub fn case_needle(needle: &str) -> Vec<(u8, u8)> {
		needle
			.as_bytes()
			.iter()
			.map(|&c| (c.to_ascii_uppercase(), c.to_ascii_lowercase()))
			.collect()
	}

	pub fn match_haystack(&self, haystack: &[u8]) -> bool {
		self.match_haystack_runtime_detection::<true, true, false>(haystack)
	}

	pub fn match_haystack_insensitive(&self, haystack: &[u8]) -> bool {
		self.match_haystack_runtime_detection::<true, false, false>(haystack)
	}

	pub fn match_haystack_unordered(&self, haystack: &[u8]) -> bool {
		self.match_haystack_runtime_detection::<false, true, false>(haystack)
	}

	pub fn match_haystack_unordered_insensitive(&self, haystack: &[u8]) -> bool {
		self.match_haystack_runtime_detection::<false, false, false>(haystack)
	}

	pub fn match_haystack_unordered_typos(&self, haystack: &[u8]) -> bool {
		self.match_haystack_runtime_detection::<false, true, true>(haystack)
	}

	pub fn match_haystack_unordered_typos_insensitive(&self, haystack: &[u8]) -> bool {
		self.match_haystack_runtime_detection::<false, false, true>(haystack)
	}

	#[inline(always)]
	fn match_haystack_runtime_detection<
		const ORDERED: bool,
		const CASE_SENSITIVE: bool,
		const TYPOS: bool,
	>(
		&self,
		haystack: &[u8],
	) -> bool {
		match haystack.len() {
			0 => return true,
			1..8 => {
				return self.match_haystack_scalar::<ORDERED, CASE_SENSITIVE, TYPOS>(haystack);
			}
			_ => {}
		}

		match (self.has_avx2, self.has_sse2) {
			#[cfg(target_arch = "x86_64")]
			(true, _) => unsafe { self.match_haystack_avx2::<ORDERED, CASE_SENSITIVE, TYPOS>(haystack) },
			#[cfg(target_arch = "x86_64")]
			(_, true) => unsafe { self.match_haystack_sse2::<ORDERED, CASE_SENSITIVE, TYPOS>(haystack) },
			_ => self.match_haystack_simd::<ORDERED, CASE_SENSITIVE, TYPOS>(haystack),
		}
	}

	#[inline(always)]
	fn match_haystack_scalar<const ORDERED: bool, const CASE_SENSITIVE: bool, const TYPOS: bool>(
		&self,
		haystack: &[u8],
	) -> bool {
		match (TYPOS, CASE_SENSITIVE) {
			(true, true) => {
				scalar::match_haystack_typos(self.needle.as_bytes(), haystack, self.max_typos)
			}
			(true, false) => scalar::match_haystack_typos_insensitive(
				&self.needle_cased,
				haystack,
				self.max_typos,
			),
			(false, true) => scalar::match_haystack(self.needle.as_bytes(), haystack),
			(false, false) => scalar::match_haystack_insensitive(&self.needle_cased, haystack),
		}
	}

	#[inline(always)]
	fn match_haystack_simd<const ORDERED: bool, const CASE_SENSITIVE: bool, const TYPOS: bool>(
		&self,
		haystack: &[u8],
	) -> bool {
		match (ORDERED, CASE_SENSITIVE, TYPOS) {
			(true, _, true) => panic!("ordered typos implementations are not yet available"),
			(true, true, false) => simd::match_haystack(self.needle.as_bytes(), haystack),
			(true, false, false) => simd::match_haystack_insensitive(&self.needle_cased, haystack),

			(false, true, false) => {
				simd::match_haystack_unordered(self.needle.as_bytes(), haystack)
			}
			(false, true, true) => simd::match_haystack_unordered_typos(
				self.needle.as_bytes(),
				haystack,
				self.max_typos,
			),
			(false, false, false) => {
				simd::match_haystack_unordered_insensitive(&self.needle_cased, haystack)
			}
			(false, false, true) => simd::match_haystack_unordered_typos_insensitive(
				&self.needle_cased,
				haystack,
				self.max_typos,
			),
		}
	}

	#[cfg(target_arch = "x86_64")]
	#[inline(always)]
	unsafe fn match_haystack_x86_64<
		const ORDERED: bool,
		const CASE_SENSITIVE: bool,
		const TYPOS: bool,
		const AVX2: bool,
	>(
		&self,
		haystack: &[u8],
	) -> bool {
		unsafe {
			match (ORDERED, CASE_SENSITIVE, TYPOS) {
				(true, _, true) => panic!("ordered typos implementations are not yet available"),
				(true, true, false) => x86_64::match_haystack(self.needle.as_bytes(), haystack),
				(true, false, false) => {
					x86_64::match_haystack_insensitive(&self.needle_cased, haystack)
				}

				(false, true, false) => {
					x86_64::match_haystack_unordered(self.needle.as_bytes(), haystack)
				}
				(false, true, true) => x86_64::match_haystack_unordered_typos(
					self.needle.as_bytes(),
					haystack,
					self.max_typos,
				),
				(false, false, false) => {
					if AVX2 {
						x86_64::match_haystack_unordered_insensitive_avx2(
							self.needle_avx2.as_ref().unwrap(),
							haystack,
						)
					} else {
						x86_64::match_haystack_unordered_insensitive(&self.needle_cased, haystack)
					}
				}
				(false, false, true) => {
					if AVX2 {
						x86_64::match_haystack_unordered_typos_insensitive_avx2(
							self.needle_avx2.as_ref().unwrap(),
							haystack,
							self.max_typos,
						)
					} else {
						x86_64::match_haystack_unordered_typos_insensitive(
							&self.needle_cased,
							haystack,
							self.max_typos,
						)
					}
				}
			}
		}
	}

	#[cfg(target_arch = "x86_64")]
	#[target_feature(enable = "sse2,avx,avx2")]
	unsafe fn match_haystack_avx2<
		const ORDERED: bool,
		const CASE_SENSITIVE: bool,
		const TYPOS: bool,
	>(
		&self,
		haystack: &[u8],
	) -> bool {
		unsafe { self.match_haystack_x86_64::<ORDERED, CASE_SENSITIVE, TYPOS, true>(haystack) }
	}

	#[cfg(target_arch = "x86_64")]
	#[target_feature(enable = "sse2")]
	unsafe fn match_haystack_sse2<
		const ORDERED: bool,
		const CASE_SENSITIVE: bool,
		const TYPOS: bool,
	>(
		&self,
		haystack: &[u8],
	) -> bool {
		unsafe { self.match_haystack_x86_64::<ORDERED, CASE_SENSITIVE, TYPOS, false>(haystack) }
	}
}

#[cfg(test)]
mod tests;
