use std::fs;
use std::path::{Path, PathBuf};

use xeno_primitives::{Key, KeyCode, Modifiers, Selection};

use super::{FilePickerOverlay, PickerQueryMode};
use crate::completion::{CompletionFileMeta, CompletionItem, CompletionKind};

fn key_tab() -> Key {
	Key {
		code: KeyCode::Tab,
		modifiers: Modifiers::NONE,
	}
}

fn completion_item(insert_text: &str, detail: &str, right: &str) -> CompletionItem {
	let file_kind = if right == "dir" {
		xeno_buffer_display::FileKind::Directory
	} else {
		xeno_buffer_display::FileKind::File
	};
	CompletionItem {
		label: insert_text.to_string(),
		insert_text: insert_text.to_string(),
		detail: Some(detail.to_string()),
		filter_text: None,
		kind: CompletionKind::File,
		match_indices: None,
		right: Some(right.to_string()),
		file: Some(CompletionFileMeta::new(insert_text, file_kind)),
	}
}

#[test]
fn query_mode_uses_path_mode_for_relative_parent_queries() {
	assert_eq!(FilePickerOverlay::query_mode("../src"), PickerQueryMode::Path);
	assert_eq!(FilePickerOverlay::query_mode("/tmp"), PickerQueryMode::Path);
	assert_eq!(FilePickerOverlay::query_mode("theme"), PickerQueryMode::Indexed);
}

#[test]
fn split_path_query_preserves_directory_prefix() {
	let (dir_part, file_part) = FilePickerOverlay::split_path_query("../src/ma");
	assert_eq!(dir_part, "../src/");
	assert_eq!(file_part, "ma");
}

#[test]
fn split_path_query_tilde_maps_to_home_directory_prefix() {
	let (dir_part, file_part) = FilePickerOverlay::split_path_query("~");
	assert_eq!(dir_part, "~/");
	assert!(file_part.is_empty());
}

#[test]
fn resolve_user_path_returns_absolute_normalized_path() {
	let temp_dir = tempfile::tempdir().expect("create tempdir");
	let root = temp_dir.path().join("workspace").join("project");
	fs::create_dir_all(&root).expect("create workspace root");

	let mut picker = FilePickerOverlay::new(None);
	picker.root = Some(root.clone());

	let resolved = picker.resolve_user_path("../outside/./file.txt");
	let expected = PathBuf::from(root.parent().expect("project has parent")).join("outside").join("file.txt");
	assert_eq!(resolved, expected);
	assert!(resolved.is_absolute());
}

#[test]
fn build_path_items_supports_parent_traversal_outside_picker_root() {
	let temp_dir = tempfile::tempdir().expect("create tempdir");
	let root = temp_dir.path().join("workspace").join("project");
	fs::create_dir_all(&root).expect("create project root");
	let outside_file = temp_dir.path().join("workspace").join("outside.rs");
	fs::write(&outside_file, "outside").expect("write outside file");

	let mut picker = FilePickerOverlay::new(None);
	picker.root = Some(root);

	let items = picker.build_path_items("../out");
	assert!(items.iter().any(|item| item.insert_text == "../outside.rs"));
}

#[test]
fn build_path_items_supports_absolute_queries() {
	let temp_dir = tempfile::tempdir().expect("create tempdir");
	let abs_root = temp_dir.path().join("absolute");
	fs::create_dir_all(&abs_root).expect("create absolute root");
	fs::write(abs_root.join("alpha.txt"), "alpha").expect("write alpha");

	let mut picker = FilePickerOverlay::new(None);
	picker.root = Some(temp_dir.path().join("workspace"));

	let query = format!("{}/al", abs_root.to_string_lossy());
	let items = picker.build_path_items(&query);
	assert!(items.iter().any(|item| item.insert_text.ends_with("alpha.txt")));
}

#[test]
fn build_path_items_uses_directory_suffix_and_hidden_filtering() {
	let temp_dir = tempfile::tempdir().expect("create tempdir");
	let root = temp_dir.path().join("workspace");
	fs::create_dir_all(root.join("src")).expect("create src dir");
	fs::write(root.join(".secret"), "hidden").expect("write hidden file");

	let mut picker = FilePickerOverlay::new(None);
	picker.root = Some(root);

	let visible_items = picker.build_path_items("./");
	assert!(visible_items.iter().any(|item| item.insert_text == "./src/"));
	assert!(!visible_items.iter().any(|item| item.insert_text.ends_with(".secret")));

	let hidden_items = picker.build_path_items("./.");
	assert!(
		hidden_items
			.iter()
			.any(|item| Path::new(&item.insert_text).file_name().is_some_and(|name| name == ".secret"))
	);
}

#[test]
fn tab_applies_selected_item_without_committing_picker() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_file_picker());

	let input_view = editor
		.state
		.overlay_system
		.interaction()
		.active()
		.map(|active| active.session.input)
		.expect("file picker input should exist");

	{
		let input = editor.state.core.buffers.get_buffer_mut(input_view).expect("picker input buffer should exist");
		input.reset_content("");
		input.set_cursor_and_selection(0, Selection::point(0));
	}

	let state = editor.overlays_mut().get_or_default::<crate::completion::CompletionState>();
	state.active = true;
	state.items = vec![completion_item("src/main.rs", "file", "file")];
	state.selected_idx = Some(0);
	state.selection_intent = crate::completion::SelectionIntent::Manual;

	let _ = futures::executor::block_on(editor.handle_key(key_tab()));

	let text = editor
		.state
		.core
		.buffers
		.get_buffer(input_view)
		.expect("picker input buffer should exist")
		.with_doc(|doc| doc.content().to_string())
		.trim_end_matches('\n')
		.to_string();
	assert_eq!(text, "src/main.rs");
	assert!(editor.state.invocation_mailbox.is_empty(), "tab completion should not commit queued commands");
	assert!(editor.state.overlay_system.interaction().is_open(), "picker should stay open after Tab");
}

#[test]
fn tab_with_no_completion_does_not_insert_literal_tab() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_file_picker());

	let input_view = editor
		.state
		.overlay_system
		.interaction()
		.active()
		.map(|active| active.session.input)
		.expect("file picker input should exist");

	{
		let input = editor.state.core.buffers.get_buffer_mut(input_view).expect("picker input buffer should exist");
		input.reset_content("");
		input.set_cursor_and_selection(0, Selection::point(0));
	}

	let state = editor.overlays_mut().get_or_default::<crate::completion::CompletionState>();
	*state = crate::completion::CompletionState::default();

	let _ = futures::executor::block_on(editor.handle_key(key_tab()));

	let text = editor
		.state
		.core
		.buffers
		.get_buffer(input_view)
		.expect("picker input buffer should exist")
		.with_doc(|doc| doc.content().to_string())
		.trim_end_matches('\n')
		.to_string();
	assert_eq!(text, "");
}

#[test]
fn tab_cycles_to_next_completion_when_input_matches_active_selection() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_file_picker());

	let input_view = editor
		.state
		.overlay_system
		.interaction()
		.active()
		.map(|active| active.session.input)
		.expect("file picker input should exist");

	{
		let input = editor.state.core.buffers.get_buffer_mut(input_view).expect("picker input buffer should exist");
		input.reset_content("src/main.rs");
		input.set_cursor_and_selection(11, Selection::point(11));
	}

	let state = editor.overlays_mut().get_or_default::<crate::completion::CompletionState>();
	state.active = true;
	state.items = vec![completion_item("src/main.rs", "file", "file"), completion_item("src/lib.rs", "file", "file")];
	state.selected_idx = Some(0);
	state.selection_intent = crate::completion::SelectionIntent::Manual;

	let _ = futures::executor::block_on(editor.handle_key(key_tab()));

	let text = editor
		.state
		.core
		.buffers
		.get_buffer(input_view)
		.expect("picker input buffer should exist")
		.with_doc(|doc| doc.content().to_string())
		.trim_end_matches('\n')
		.to_string();
	assert_eq!(text, "src/lib.rs");
}
