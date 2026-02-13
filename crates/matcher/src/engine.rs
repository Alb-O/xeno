use crate::Config;
use crate::prefilter::Prefilter;

pub(crate) struct CandidateFilter {
	min_haystack_len: usize,
	max_typos: Option<u16>,
	prefilter: Option<Prefilter>,
}

impl CandidateFilter {
	pub fn new(needle: &str, config: &Config) -> Self {
		let max_typos = config.max_typos;
		let min_haystack_len = max_typos.map(|max| needle.len().saturating_sub(max as usize)).unwrap_or(0);
		let use_prefilter = config.prefilter && max_typos.is_some();
		let prefilter = use_prefilter.then(|| Prefilter::new(needle, max_typos.unwrap_or(0)));

		Self {
			min_haystack_len,
			max_typos,
			prefilter,
		}
	}

	#[inline]
	pub fn allows(&self, haystack: &str) -> bool {
		if haystack.len() < self.min_haystack_len {
			return false;
		}

		let Some(prefilter) = &self.prefilter else {
			return true;
		};

		match self.max_typos {
			Some(0) => prefilter.match_haystack_unordered_insensitive(haystack.as_bytes()),
			Some(_) => prefilter.match_haystack_unordered_typos_insensitive(haystack.as_bytes()),
			None => true,
		}
	}
}
