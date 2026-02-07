//! Proofs for capability provider invariants.

/// Verifies the Delegator Rule via compile-fail assertion.
///
/// This test is primarily implemented as a rustdoc `compile_fail` block
/// in the anchor file to ensure it fails at compile time if regressed.
#[cfg_attr(test, test)]
pub fn test_delegator_rule() {
	// Logic-only verification is handled by the rustdoc compile_fail tripwire.
}

/// Verifies that capability mutations enqueue layer events.
#[cfg_attr(test, test)]
pub fn test_mutation_side_effect_invariant() {
	use xeno_primitives::range::CharIdx;
	use xeno_registry::CursorAccess;

	use crate::impls::Editor;

	let mut ed = Editor::new_scratch();
	{
		let mut caps = ed.caps();
		caps.set_cursor(CharIdx::from(10usize));
	}

	let drained = ed.state.effects.drain();
	assert!(
		!drained.layer_events.is_empty(),
		"Mutation must enqueue a layer event"
	);
}
