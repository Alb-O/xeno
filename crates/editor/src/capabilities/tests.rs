use xeno_registry::commands::CommandEditorOps;

use super::*;

#[test]
fn test_setlocal_rejects_global_scoped_option() {
	let mut editor = Editor::new_scratch();
	let result = editor.set_local_option("theme", "gruvbox");
	assert!(result.is_err());
	let err = result.unwrap_err();
	assert!(
		err.to_string().contains("global option"),
		"Expected error about global option, got: {}",
		err
	);
}

#[test]
fn test_setlocal_accepts_buffer_scoped_option() {
	let mut editor = Editor::new_scratch();
	let result = editor.set_local_option("tab-width", "2");
	assert!(result.is_ok(), "Expected success, got: {:?}", result);
}

#[test]
fn test_setlocal_rejects_unknown_option() {
	let mut editor = Editor::new_scratch();
	let result = editor.set_local_option("nonexistent-option", "value");
	assert!(result.is_err());
	let err = result.unwrap_err();
	assert!(
		err.to_string().contains("unknown option"),
		"Expected error about unknown option, got: {}",
		err
	);
}
