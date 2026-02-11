use super::{exceeds_typo_budget, match_too_large};
use crate::smith_waterman::greedy::match_greedy;
use crate::smith_waterman::reference::{char_indices_from_score_matrix, smith_waterman, typos_from_score_matrix};
use crate::{Config, MatchIndices};

/// Gets the matched indices for the needle on a single haystack.
///
/// You should call this sparingly, as it uses an unoptimized smith waterman implementation. For
/// example, if you're writing a fuzzy matcher UI, you would only call this for the items visible
/// on screen.
pub fn match_indices<S1: AsRef<str>, S2: AsRef<str>>(needle: S1, haystack: S2, config: &Config) -> Option<MatchIndices> {
	let needle = needle.as_ref();
	let haystack = haystack.as_ref();

	// Fallback to greedy matching
	if match_too_large(needle, haystack) {
		let (score, indices, exact) = match_greedy(needle, haystack, &config.scoring);
		if score == 0 || exceeds_typo_budget(config.max_typos, needle, indices.len()) {
			return None;
		}
		return Some(MatchIndices { score, indices, exact });
	}

	// Get score matrix
	let (score, score_matrix, exact) = smith_waterman(needle, haystack, &config.scoring);
	let score_matrix_ref = score_matrix.iter().map(|v| v.as_slice()).collect::<Vec<_>>();

	// Ensure there's not too many typos
	if let Some(max_typos) = config.max_typos {
		let typos = typos_from_score_matrix(&score_matrix_ref);
		if typos > max_typos {
			return None;
		}
	}

	let indices = char_indices_from_score_matrix(&score_matrix_ref);

	Some(MatchIndices { score, indices, exact })
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn greedy_fallback_respects_max_typos() {
		let needle = "z".repeat(4096);
		let haystack = "a".repeat(5000);
		let config = Config {
			max_typos: Some(0),
			..Config::default()
		};

		assert!(match_indices(&needle, &haystack, &config).is_none());
	}

	#[test]
	fn greedy_fallback_returns_exact_when_in_budget() {
		let needle = "a".repeat(2000);
		let haystack = "a".repeat(3000);
		let config = Config {
			max_typos: Some(0),
			..Config::default()
		};

		assert!(match_indices(&needle, &haystack, &config).is_some());
	}
}
