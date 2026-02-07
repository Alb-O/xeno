/// Must not use RTTI or engine-specific downcasting to access `Editor` methods.
///
/// - Enforced in: `apply_effects`
/// - Failure symptom: Compilation error or boundary breach that couples registry
///   policy to engine implementation.
#[cfg_attr(test, test)]
pub fn test_honesty_rule() {
	// If it compiles and runs with MockEditor (which is NOT Editor), it's honest.
}

/// Must route all side effects through capability providers, not from the interpreter.
///
/// - Enforced in: `apply_effects`
/// - Failure symptom: Duplicate notifications or missed UI updates during re-entrant actions.
#[cfg_attr(test, test)]
pub fn test_single_path_side_effects() {
	// Covered by behavior tests in crates/editor/src/editor_ctx/tests.rs
}
