/// Must not implement `xeno_registry::*Access` traits directly on `Editor`.
///
/// - Enforced in: `EditorCaps` (via delegator pattern)
/// - Failure symptom: Circular dependencies or accidental leakage of engine-specific
///   methods into the action registry.
#[cfg_attr(test, test)]
pub fn test_delegator_rule() {
	// Logic-only verification is handled by the rustdoc compile_fail tripwire.
}

/// Must enqueue layer events for capability mutations.
///
/// - Enforced in: `EditorCaps` (via domain-specific implementations)
/// - Failure symptom: UI layers (overlays, status bars) failing to update after an
///   action executes.
#[cfg_attr(test, test)]
pub fn test_mutation_side_effect_invariant() {
	use xeno_primitives::range::CharIdx;
	use xeno_registry::actions::CursorAccess;

	use crate::Editor;

	let mut ed = Editor::new_scratch();
	{
		let mut caps = ed.caps();
		caps.set_cursor(CharIdx::from(10usize));
	}

	let drained = ed.state.effects.drain();
	assert!(!drained.layer_events.is_empty(), "Mutation must enqueue a layer event");
}
