//! Shared winner-selection helpers for precedence-ranked domain lookups.

/// Promotes `candidate` to winner when no winner exists yet or candidate outranks current winner.
pub fn promote_if_winner<Id>(winner: &mut Option<(Id, crate::core::Party)>, candidate_id: Id, candidate_party: crate::core::Party)
where
	Id: Copy,
{
	match winner {
		None => {
			*winner = Some((candidate_id, candidate_party));
		}
		Some((_, best_party)) => {
			if crate::core::index::precedence::party_wins(&candidate_party, best_party) {
				*winner = Some((candidate_id, candidate_party));
			}
		}
	}
}
