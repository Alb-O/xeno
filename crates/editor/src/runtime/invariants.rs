use std::time::Duration;

use xeno_primitives::{Key, KeyCode};

use super::{CursorStyle, RuntimeEvent};
use crate::Editor;

/// Must execute one maintenance `pump` after handling each runtime event.
///
/// * Enforced in: `Editor::on_event`
/// * Failure symptom: input handlers mutate state without advancing deferred work.
#[tokio::test]
async fn test_on_event_implies_single_pump_cycle() {
	let mut editor = Editor::new_scratch();
	let _ = editor.pump().await;

	let directive = editor.on_event(RuntimeEvent::Key(Key::char('i'))).await;
	assert_eq!(directive.poll_timeout, Some(Duration::from_millis(16)));
}

/// Must defer overlay commit execution to `pump` via pending-commit flag.
///
/// * Enforced in: `Editor::handle_key_active`, `Editor::pump`
/// * Failure symptom: overlay commit runs re-entrantly inside key handling.
#[tokio::test]
async fn test_overlay_commit_deferred_until_pump() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let _ = editor.handle_key(Key::new(KeyCode::Enter)).await;
	assert!(editor.frame().pending_overlay_commit);
	assert!(editor.overlay_kind().is_some());

	let _ = editor.pump().await;
	assert!(!editor.frame().pending_overlay_commit);
	assert!(editor.overlay_kind().is_none());
}

/// Must default cursor style to Beam in insert mode and Block otherwise.
///
/// * Enforced in: `Editor::derive_cursor_style`
/// * Failure symptom: frontends render incorrect cursor shape for modal state.
#[cfg_attr(test, test)]
pub(crate) fn test_cursor_style_defaults_follow_mode() {
	let mut editor = Editor::new_scratch();
	assert_eq!(editor.derive_cursor_style(), CursorStyle::Block);

	editor.set_mode(xeno_primitives::Mode::Insert);
	assert_eq!(editor.derive_cursor_style(), CursorStyle::Beam);
}
