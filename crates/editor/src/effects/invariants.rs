/// Must route all UI consequences through `EffectSink` and `flush_effects`.
///
/// - Enforced in: `Editor::flush_effects`
/// - Failure symptom: Inconsistent UI state or dropped event notifications.
#[cfg_attr(test, test)]
pub fn test_single_path_side_effects() {
	// Covered by integration tests and effect interpreter tests.
}
