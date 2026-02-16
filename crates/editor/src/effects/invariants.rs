/// Must route all UI consequences through `EffectSink` and `flush_effects`.
///
/// * Enforced in: `Editor::flush_effects`
/// * Failure symptom: Inconsistent UI state or dropped event notifications.
#[cfg_attr(test, test)]
pub fn test_single_path_side_effects() {
	// Covered by integration tests and effect interpreter tests.
}

/// Must enqueue commit-close overlay requests as deferred runtime work.
///
/// * Enforced in: `Editor::handle_overlay_request`
/// * Failure symptom: commit closes execute re-entrantly during synchronous effect flush.
#[cfg_attr(test, test)]
pub fn test_commit_close_enqueues_deferred_overlay_commit() {
	use xeno_registry::actions::editor_ctx::{OverlayCloseReason, OverlayRequest};

	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());
	assert!(!editor.frame().deferred_work.has_overlay_commit());

	let result = editor.handle_overlay_request(OverlayRequest::CloseModal {
		reason: OverlayCloseReason::Commit,
	});
	assert!(result.is_ok());
	assert!(editor.state.overlay_system.interaction().is_open());
	assert!(editor.frame().deferred_work.has_overlay_commit());
}
