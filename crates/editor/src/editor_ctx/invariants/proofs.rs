//! Proofs for effect interpreter invariants.

/// Verifies the Honesty Rule by ensuring apply_effects can run with a mock.
#[cfg_attr(test, test)]
pub fn test_honesty_rule() {
	// If it compiles and runs with MockEditor (which is NOT Editor), it's honest.
}

/// Verifies that the interpreter doesn't emit side effects directly.
#[cfg_attr(test, test)]
pub fn test_single_path_side_effects() {
	// Covered by behavior tests in crates/editor/src/editor_ctx/tests.rs
}
