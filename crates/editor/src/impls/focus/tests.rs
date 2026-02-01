use super::*;
use crate::impls::Editor;

#[test]
fn test_set_focus_normalizes_invalid_buffer() {
	let mut editor = Editor::new_scratch();
	let buffer1 = editor.focused_view();
	let base_id = editor.state.windows.base_id();

	// Create a second buffer (it won't be in layout)
	let buffer2 = editor.state.core.buffers.create_scratch();

	// Manually set focus to buffer2 to simulate stale focus
	// (In real code, this shouldn't happen due to normalization,
	// but we're testing the repair mechanism)
	editor.state.focus = FocusTarget::Buffer {
		window: base_id,
		buffer: buffer2,
	};
	editor.base_window_mut().focused_buffer = buffer2;

	// Verify we have stale focus
	assert_eq!(editor.focused_view(), buffer2);

	// Remove buffer2 to make it truly invalid
	editor.finalize_buffer_removal(buffer2);

	// Try to focus the now-invalid buffer2 - should normalize to buffer1
	let result = editor.set_focus(
		FocusTarget::Buffer {
			window: base_id,
			buffer: buffer2,
		},
		FocusReason::Programmatic,
	);

	// Should have changed focus (normalized to valid buffer)
	assert!(result, "set_focus should return true when focus changes");

	// Check state.focus directly
	assert_eq!(
		editor.state.focus,
		FocusTarget::Buffer {
			window: base_id,
			buffer: buffer1
		},
		"state.focus should be normalized to buffer1"
	);

	// Then check focused_view()
	assert_eq!(
		editor.focused_view(),
		buffer1,
		"focused_view() should return buffer1"
	);
	assert!(
		editor
			.state
			.core
			.buffers
			.get_buffer(editor.focused_view())
			.is_some()
	);
}

#[test]
fn test_set_focus_creates_scratch_when_no_buffers() {
	let mut editor = Editor::new_scratch();
	let buffer1 = editor.focused_view();

	// Remove all buffers
	editor.finalize_buffer_removal(buffer1);

	// Try to focus invalid buffer - should create scratch
	let result = editor.set_focus(
		FocusTarget::Buffer {
			window: editor.state.windows.base_id(),
			buffer: buffer1,
		},
		FocusReason::Programmatic,
	);

	// Should have changed focus (created scratch buffer)
	assert!(result);
	let new_focus = editor.focused_view();
	assert!(new_focus != buffer1);
	assert!(editor.state.core.buffers.get_buffer(new_focus).is_some());
}
