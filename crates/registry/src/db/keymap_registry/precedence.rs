//! Candidate precedence policy for slot conflict resolution.

use std::cmp::Ordering;

use super::spec::KeymapBindingSource;

/// Comparable precedence tuple for one slot candidate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CandidatePrecedence<'a> {
	pub source: KeymapBindingSource,
	pub ordinal: usize,
	pub priority: i16,
	pub target_desc: &'a str,
}

/// Compare two candidates and return ordering where `Greater` wins.
///
/// Policy:
/// * Source precedence: Override > Preset > RuntimeAction > ActionDefault.
/// * For ActionDefault/RuntimeAction, lower numeric priority wins.
/// * For Preset/Override, last-writer ordinal wins.
/// * Stable tie-break uses target description.
pub(crate) fn compare_candidates(a: CandidatePrecedence<'_>, b: CandidatePrecedence<'_>) -> Ordering {
	let by_rank = a.source.rank().cmp(&b.source.rank());
	if by_rank != Ordering::Equal {
		return by_rank;
	}

	match a.source {
		KeymapBindingSource::ActionDefault | KeymapBindingSource::RuntimeAction => b
			.priority
			.cmp(&a.priority)
			.then_with(|| a.target_desc.cmp(b.target_desc))
			.then_with(|| b.ordinal.cmp(&a.ordinal)),
		KeymapBindingSource::Preset | KeymapBindingSource::Override => a.ordinal.cmp(&b.ordinal).then_with(|| a.target_desc.cmp(b.target_desc)),
	}
}
