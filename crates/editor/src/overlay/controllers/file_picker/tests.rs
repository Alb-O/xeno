use std::fs;
use std::path::{Path, PathBuf};

use super::{FilePickerOverlay, PickerQueryMode};

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
