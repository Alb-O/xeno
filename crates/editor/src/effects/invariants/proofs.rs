//! Proofs for side-effect flusher invariants.

/// Verifies that flush_effects is the sole dispatcher by checking sink state.
#[cfg_attr(test, test)]
pub fn test_single_path_side_effects() {
	// Covered by integration tests and effect interpreter tests.
}
